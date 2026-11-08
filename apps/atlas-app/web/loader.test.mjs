import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import { loadBytes, loadPublication } from './loader.mjs';

test('publication loads valid UUID shards and rejects repeated or misfiled identities', async t => {
  const documents = new Map([
    ['/data/sources.json', []], ['/data/entities/shards.json', ['ab']],
    ['/data/entities/aliases.json', { 'old-entity': 'ab-entity' }],
    ['/data/entities/index/ab.json', { 'ab-entity': { entity: { id: 'ab-entity' } } }],
  ]);
  t.mock.method(globalThis, 'fetch', async url => new Response(JSON.stringify(documents.get(url.pathname))));
  const base = new URL('https://atlas.example/data/');
  const publication = await loadPublication(base);
  assert.equal(Object.keys(publication.entries).length, 1);
  assert.deepEqual(publication.aliases, { 'old-entity': 'ab-entity' });
  documents.set('/data/entities/shards.json', ['ab', 'ab']);
  await assert.rejects(loadPublication(base), /repeated entity/);
  documents.set('/data/entities/shards.json', ['../private']);
  await assert.rejects(loadPublication(base), /Invalid entity shard/);
  documents.set('/data/entities/shards.json', ['ab']);
  documents.set('/data/entities/index/ab.json', { 'cd-entity': {} });
  await assert.rejects(loadPublication(base), /Invalid or repeated entity/);
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
