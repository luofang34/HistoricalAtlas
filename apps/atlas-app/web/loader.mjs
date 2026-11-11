const MAX_PART_BYTES = 4_000_000;
const decoder = new TextDecoder();

export async function json(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response.json();
}

async function verifyPart(part, base, label = 'publication part') {
  if (!part || typeof part.path !== 'string' || part.path.startsWith('/') ||
      part.path.includes('..') || part.path.includes('\\') || !/^[a-zA-Z0-9/_.-]+$/.test(part.path)) {
    throw new Error(`Invalid ${label} path`);
  }
  if (!Number.isInteger(part.bytes) || part.bytes <= 0 || part.bytes >= MAX_PART_BYTES) {
    throw new Error(`Invalid ${label} size: ${part.path}`);
  }
  if (typeof part.sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(part.sha256)) {
    throw new Error(`Invalid ${label} digest: ${part.path}`);
  }
  const response = await fetch(new URL(part.path, base));
  if (!response.ok) throw new Error(`${part.path}: HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length !== part.bytes) throw new Error(`Incomplete ${label} ${part.path}`);
  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
  const hex = Array.from(hash, value => value.toString(16).padStart(2, '0')).join('');
  if (hex !== part.sha256) throw new Error(`${label} checksum differs: ${part.path}`);
  return bytes;
}

async function loadVecParts(parts, base, label) {
  if (!Array.isArray(parts) || parts.length === 0) throw new Error(`Empty ${label} manifest`);
  const items = [];
  for (let index = 0; index < parts.length; index += 8) {
    const batch = await Promise.all(parts.slice(index, index + 8).map(async part => {
      const bytes = await verifyPart(part, base, label);
      const parsed = JSON.parse(decoder.decode(bytes));
      if (!Array.isArray(parsed)) throw new Error(`Expected JSON array in ${part.path}`);
      return parsed;
    }));
    for (const chunk of batch) items.push(...chunk);
  }
  return items;
}

async function loadMapParts(parts, base, label) {
  if (!Array.isArray(parts) || parts.length === 0) throw new Error(`Empty ${label} manifest`);
  const merged = {};
  for (let index = 0; index < parts.length; index += 8) {
    const batch = await Promise.all(parts.slice(index, index + 8).map(async part => {
      const bytes = await verifyPart(part, base, label);
      const parsed = JSON.parse(decoder.decode(bytes));
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        throw new Error(`Expected JSON object in ${part.path}`);
      }
      return parsed;
    }));
    for (const chunk of batch) {
      for (const [key, value] of Object.entries(chunk)) {
        if (Object.hasOwn(merged, key)) throw new Error(`Duplicate key ${key} in ${label}`);
        merged[key] = value;
      }
    }
  }
  return merged;
}

async function loadConcatenatedJson(parts, base, label) {
  if (!Array.isArray(parts) || parts.length === 0) throw new Error(`Empty ${label} manifest`);
  const buffers = await Promise.all(parts.map(part => verifyPart(part, base, label)));
  const combined = new Uint8Array(buffers.reduce((sum, part) => sum + part.length, 0));
  let offset = 0;
  for (const buffer of buffers) { combined.set(buffer, offset); offset += buffer.length; }
  return JSON.parse(decoder.decode(combined));
}

export async function loadPublication(base) {
  const manifest = await json(new URL('manifest.json', base));
  if (!manifest || manifest.schema_version !== 1) {
    throw new Error(`Unsupported publication schema version: ${manifest?.schema_version}`);
  }
  if (!Array.isArray(manifest.catalog_shards) || manifest.catalog_shards.length === 0) {
    throw new Error('Empty catalog shard manifest');
  }
  const [sources, aliases, published, shards, catalog] = await Promise.all([
    loadVecParts(manifest.sources, base, 'sources'),
    loadMapParts(manifest.aliases, base, 'aliases'),
    loadVecParts(manifest.published, base, 'published'),
    loadVecParts(manifest.shard_inventories, base, 'shard inventory'),
    loadVecParts(manifest.catalog_shards.map(entry => entry?.part), base, 'catalog shard'),
  ]);
  if (!Array.isArray(manifest.plate_models)) throw new Error('Invalid plate model manifest');
  const seenModels = new Set();
  const plate_models = {};
  for (const model of manifest.plate_models) {
    if (!model || typeof model.id !== 'string' || !/^[a-z0-9-]+$/.test(model.id) || seenModels.has(model.id)) {
      throw new Error('Invalid plate model manifest');
    }
    seenModels.add(model.id);
    plate_models[model.id] = await loadConcatenatedJson(model.parts, base, 'plate model');
  }
  return { manifest, sources, aliases, published, shards, catalog, plate_models };
}

export async function loadBytes(manifest, base = import.meta.url) {
  const parts = await Promise.all(manifest.parts.map(async part => {
    if (!/^[a-zA-Z0-9_-]+\.part$/.test(part.path)) throw new Error('Invalid WASM part path');
    return verifyPart(part, base, 'WASM part');
  }));
  const bytes = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0;
  for (const part of parts) { bytes.set(part, offset); offset += part.length; }
  return bytes;
}

export async function loadModule() {
  const manifest = await json(new URL('./runtime.json', import.meta.url));
  if (!/^[a-zA-Z0-9_-]+\.js$/.test(manifest.module)) throw new Error('Invalid WASM module path');
  const [runtime, bytes] = await Promise.all([
    import(new URL(manifest.module, import.meta.url).href), loadBytes(manifest),
  ]);
  await runtime.default({ module_or_path: bytes });
  return runtime;
}
