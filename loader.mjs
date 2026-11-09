export async function json(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response.json();
}

export async function loadPublication(base) {
  const [sources, shards, aliases] = await Promise.all([
    json(new URL('sources.json', base)), json(new URL('entities/shards.json', base)),
    json(new URL('entities/aliases.json', base)),
  ]);
  if (!Array.isArray(shards) || shards.some(name => !/^[0-9a-f]{2}$/.test(name))) {
    throw new Error('Invalid entity shard manifest');
  }
  const entries = {};
  // Batches keep request concurrency bounded as the atlas grows.
  for (let index = 0; index < shards.length; index += 8) {
    const batch = await Promise.all(shards.slice(index, index + 8).map(async name =>
      [name, await json(new URL(`entities/index/${name}.json`, base))]));
    for (const [name, shard] of batch) {
      for (const [id, entry] of Object.entries(shard)) {
        if (!id.startsWith(name) || Object.hasOwn(entries, id)) throw new Error(`Invalid or repeated entity ${id}`);
        entries[id] = entry;
      }
    }
  }
  return { sources, entries, aliases };
}

export async function loadBytes(manifest, base = import.meta.url) {
  const parts = await Promise.all(manifest.parts.map(async part => {
    if (!/^[a-zA-Z0-9_-]+\.part$/.test(part.path)) throw new Error('Invalid WASM part path');
    const response = await fetch(new URL(part.path, base));
    if (!response.ok) throw new Error(`${part.path}: HTTP ${response.status}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.length !== part.bytes) throw new Error(`Incomplete WASM part ${part.path}`);
    const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
    const hex = Array.from(hash, value => value.toString(16).padStart(2, '0')).join('');
    if (hex !== part.sha256) throw new Error(`WASM part checksum differs: ${part.path}`);
    return bytes;
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
