import assert from 'node:assert/strict';
import { test } from 'node:test';
import { loadOverlay } from './overlay.mjs';

test('query loading preserves exact ticks and split UTF-8 while resolving relative URLs', async t => {
  const document = '{"title":"毕节","ticks":"-142324776000000000000"}';
  const bytes = new TextEncoder().encode(document);
  let requested;
  t.mock.method(globalThis, 'fetch', async (url, options) => {
    requested = url.href;
    assert.equal(options.credentials, 'omit');
    assert(options.signal instanceof AbortSignal);
    return new Response(new ReadableStream({ start(controller) {
      for (const byte of bytes) controller.enqueue(Uint8Array.of(byte));
      controller.close();
    } }));
  });
  assert.equal(await loadOverlay('https://atlas.example/view/'), null);
  assert.equal(requested, undefined);
  const result = await loadOverlay('https://atlas.example/view/?overlay=events/test.json');
  assert.equal(requested, 'https://atlas.example/view/events/test.json');
  assert.equal(result.origin, requested);
  assert.equal(result.content, document);
});

test('bad URLs and failed requests are reported independently of atlas startup', async t => {
  let requests = 0;
  t.mock.method(globalThis, 'fetch', async () => { requests += 1; return new Response('', { status: 404 }); });
  for (const value of ['', 'javascript:alert(1)', 'file:///tmp/event.json', 'data:application/json,{}']) {
    const result = await loadOverlay(`https://atlas.example/?overlay=${encodeURIComponent(value)}`);
    assert(result.error);
  }
  assert.equal(requests, 0);
  assert.match((await loadOverlay('https://atlas.example/?overlay=missing.json')).error, /HTTP 404/);
  t.mock.method(globalThis, 'fetch', async () => { throw new TypeError('CORS request failed'); });
  assert.match((await loadOverlay('https://atlas.example/?overlay=https://events.example/a.json')).error, /CORS request failed/);
});

test('the event package limit cancels an oversized response stream', async t => {
  let cancelled = false;
  t.mock.method(globalThis, 'fetch', async () => new Response(new ReadableStream({
    start(controller) { controller.enqueue(new Uint8Array(4_000_000)); },
    cancel() { cancelled = true; },
  })));
  const result = await loadOverlay('https://atlas.example/?overlay=large.json');
  assert.match(result.error, /smaller than 4 MB/);
  assert.equal(cancelled, true);
  t.mock.method(globalThis, 'fetch', async () => new Response(Uint8Array.of(0xff)));
  assert((await loadOverlay('https://atlas.example/?overlay=invalid.json')).error);
});
