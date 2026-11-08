export function connectInput(app, canvas, input) {
  let composing = false;
  input.addEventListener('compositionstart', () => { composing = true; });
  input.addEventListener('compositionend', () => {
    composing = false;
    if (input.value) app.text(input.value);
    input.value = '';
  });
  input.addEventListener('input', () => {
    if (!composing && input.value) { app.text(input.value); input.value = ''; }
  });
  input.addEventListener('paste', event => {
    event.preventDefault();
    app.paste(event.clipboardData.getData('text/plain'));
  });
  for (const type of ['keydown', 'keyup']) {
    input.addEventListener(type, event => {
      if (composing || event.isComposing) return;
      const command = event.metaKey || event.ctrlKey;
      if (event.key.length > 1 || command) {
        if (!(command && ['v', 'c', 'x'].includes(event.key.toLowerCase()))) event.preventDefault();
        app.key(event.key, type === 'keydown', event.shiftKey, command);
      }
    });
  }
  const observer = new MutationObserver(() => {
    if (input.dataset.active === 'true') input.focus({ preventScroll: true });
    else if (document.activeElement === input) canvas.focus({ preventScroll: true });
  });
  observer.observe(input, { attributes: true, attributeFilter: ['data-active'] });
  const resize = new ResizeObserver(entries => {
    for (const entry of entries) app.resize(entry.contentRect.width, entry.contentRect.height);
  });
  resize.observe(canvas);
  window.addEventListener('pagehide', () => { observer.disconnect(); resize.disconnect(); app.free(); }, { once: true });
}
