import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { loadBytes, loadPublication } from './loader.mjs';

function makePart(path, value, store) {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  store.set(`/data/${path}`, bytes);
  return {
    path,
    bytes: bytes.length,
    sha256: createHash('sha256').update(bytes).digest('hex'),
  };
}

test('bootstrap loader verifies v1 manifest checksums without fetching entity leaf shards', async t => {
  const store = new Map();
  const sourcesPart = makePart('sources/000.json', [{ id: 'src-1', title: 'Test' }], store);
  const aliasesPart = makePart('entities/aliases/000.json', { 'old-entity': 'ab-entity' }, store);
  const publishedPart = makePart('entities/published/000.json', ['ab-entity'], store);
  const shardEntry = {
    id: 'earth/overview/0_0_0',
    body: 'earth',
    overview: true,
    part: {
      path: 'entities/index/earth/overview/0_0_0.json',
      bytes: 120,
      sha256: 'a'.repeat(64),
    },
    entity_count: 1,
  };
  const shardInvPart = makePart('entities/shards/000.json', [shardEntry], store);
  const catalogFilePart = makePart('entities/catalog/all.json', [{
    id: 'ab-entity',
    key: 'earth/test',
    body: 'earth',
    kind: { place: 'settlement' },
    shard: 'earth/overview/0_0_0',
    label: 'Test Place',
  }], store);
  const platePart = makePart('plates/test-model/000.part', { id: 'test-model', sequences: [] }, store);
  const baseManifest = () => ({
    schema_version: 1,
    sources: [sourcesPart],
    aliases: [aliasesPart],
    published: [publishedPart],
    shard_inventories: [shardInvPart],
    catalog_shards: [{ prefix: 'all', part: catalogFilePart, count: 1 }],
    plate_models: [{ id: 'test-model', youngest_ma: 0, oldest_ma: 100, parts: [platePart] }],
  });
  let manifestObj = baseManifest();
  const requested = [];
  t.mock.method(globalThis, 'fetch', async url => {
    requested.push(url.pathname);
    if (url.pathname === '/data/manifest.json') {
      return new Response(JSON.stringify(manifestObj));
    }
    const payload = store.get(url.pathname);
    if (!payload) return new Response('not found', { status: 404 });
    return new Response(payload);
  });

  const base = new URL('https://atlas.example/data/');
  const bootstrap = await loadPublication(base);
  assert.equal(bootstrap.manifest.schema_version, 1);
  assert.equal(bootstrap.catalog.length, 1);
  assert.equal(bootstrap.shards.length, 1);
  assert.deepEqual(bootstrap.aliases, { 'old-entity': 'ab-entity' });
  assert.deepEqual(bootstrap.plate_models, { 'test-model': { id: 'test-model', sequences: [] } });
  assert.equal(requested.some(path => path.startsWith('/data/entities/index/')), false);

  manifestObj = { ...baseManifest(), schema_version: 2 };
  await assert.rejects(loadPublication(base), /Unsupported publication schema version: 2/);

  manifestObj = { ...baseManifest(), plate_models: [{ id: '../private', parts: [platePart] }] };
  await assert.rejects(loadPublication(base), /Invalid plate model manifest/);

  manifestObj = {
    ...baseManifest(),
    sources: [{ ...sourcesPart, path: '../secret.json' }],
  };
  await assert.rejects(loadPublication(base), /Invalid sources path/);

  manifestObj = {
    ...baseManifest(),
    sources: [{ ...sourcesPart, sha256: 'b'.repeat(64) }],
  };
  await assert.rejects(loadPublication(base), /sources checksum differs/);
});

test('WASM parts retain order and reject incomplete or corrupted responses', async t => {
  const first = Uint8Array.from([0, 97, 115, 109]);
  const second = Uint8Array.from([1, 0, 0, 0]);
  const parts = [first, second].map((bytes, index) => ({ path: `atlas-${index}.part`, bytes: bytes.length,
    sha256: createHash('sha256').update(bytes).digest('hex') }));
  const responses = new Map([['/atlas-0.part', first], ['/atlas-1.part', second]]);
  t.mock.method(globalThis, 'fetch', async url => new Response(responses.get(url.pathname)));
  const base = 'https://atlas.example/loader.mjs';
  assert.equal(WebAssembly.validate(await loadBytes({ parts }, base)), true);
  responses.set('/atlas-1.part', Uint8Array.from([1]));
  await assert.rejects(loadBytes({ parts }, base), /Incomplete WASM part/);
  responses.set('/atlas-1.part', Uint8Array.from([2, 0, 0, 0]));
  await assert.rejects(loadBytes({ parts }, base), /checksum differs/);
});
