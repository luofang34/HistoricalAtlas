const MAX_BYTES = 4_000_000;

async function packageText(response) {
  if (!response.body) throw new Error('Empty event package response');
  const reader = response.body.getReader();
  const decoder = new TextDecoder('utf-8', { fatal: true });
  const text = [];
  let bytes = 0;
  try {
    while (true) {
      const chunk = await reader.read();
      if (chunk.done) break;
      bytes += chunk.value.byteLength;
      if (bytes >= MAX_BYTES) {
        await reader.cancel();
        throw new Error('Event package must be smaller than 4 MB');
      }
      text.push(decoder.decode(chunk.value, { stream: true }));
    }
    text.push(decoder.decode());
    return text.join('');
  } finally {
    reader.releaseLock();
  }
}

// Loading is separate from atlas startup so a failed package leaves the map available.
export async function loadOverlay(pageUrl) {
  const page = new URL(pageUrl);
  const requested = page.searchParams.get('overlay');
  if (requested === null) return null;
  let origin = requested;
  try {
    if (!requested.trim()) throw new Error('Event package URL is empty');
    const url = new URL(requested, page);
    origin = url.href;
    if (!['https:', 'http:'].includes(url.protocol)) throw new Error('Event package requires an HTTP(S) URL');
    const response = await fetch(url, { credentials: 'omit', signal: AbortSignal.timeout(15_000) });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return { origin, content: await packageText(response) };
  } catch (error) {
    return { origin, error: error.message ?? String(error) };
  }
}
