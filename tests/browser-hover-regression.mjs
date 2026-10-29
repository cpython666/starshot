// Run in one existing Ego TaskSpace. Original pixels and mocked native geometry only.
const assert = (await import('node:assert/strict')).default;
const fs = await import('node:fs/promises');
const { fileURLToPath } = await import('node:url');
const config = globalThis.starshotRegressionConfig;
if (!Number.isInteger(config?.spaceId) || !config.page) throw Error('Pass an existing {spaceId,page}');
const page = (await taskSpace(config.spaceId)).page(config.page), results = [];
const dir = fileURLToPath(new URL('../artifacts/hover-detection/', import.meta.url));
await fs.mkdir(dir, { recursive: true });
// Startup cases deliberately suppress page rAF callbacks, including default polling.
const wait = fn => page.waitForFunction(fn, undefined, { timeout: 15_000, polling: 25 });
const key = (key, type = 'keydown', extra = {}) => page.evaluate(({ key, type, extra }) => window.dispatchEvent(new KeyboardEvent(type, { key, bubbles: true, cancelable: true, ...extra })), { key, type, extra });
const preload = await page.cdp('Page.addScriptToEvaluateOnNewDocument', { source: '(' + function () {
  const params = new URLSearchParams(location.search);
  const m = window._hoverMock = { queries: [], generations: [], invalidations: [], unexpected: [], held: [], presentations: [], presented: false, decoded: false, workers: 0, hold: false, enabled: params.has('controls') };
  const decode = HTMLImageElement.prototype.decode;
  HTMLImageElement.prototype.decode = async function () {
    if (!this.classList.contains('capture-source')) return decode.call(this);
    if (params.get('decode') === 'held') await new Promise(resolve => { m.releaseDecode = resolve; });
    if (params.get('decode') === 'error') throw Error('fixture: displayed image decode failed');
    await decode.call(this);
    m.decoded = this.isConnected;
  };
  if (params.has('no-raf')) { window.requestAnimationFrame = () => 1; window.cancelAnimationFrame = () => {}; }
  if (params.get('worker') === 'constructor') window.Worker = class { constructor() { throw Error('fixture: worker disabled'); } };
  if (params.get('worker') === 'async') window.Worker = class {
    postMessage(value, transfer) { structuredClone(value, { transfer }); queueMicrotask(() => this.onerror?.(new ErrorEvent('error'))); }
    terminate() {}
  };
  if (params.get('worker') === 'held') window.Worker = class {
    postMessage() { m.releaseWorker = () => this.onmessage?.({ data: [params.has('parents') ? { x: 150, y: 130, width: 200, height: 100 } : { x: 100, y: 100, width: 400, height: 240 }] }); }
    terminate() {}
  };
  const Worker = window.Worker;
  window.Worker = class {
    constructor(...args) { m.workers++; if (!m.presented) m.unexpected.push('worker-before-ready'); return new Worker(...args); }
  };
  window.__TAURI_INTERNALS__ = { async invoke(command, args = {}) {
    if (command === 'get_capture_frame') {
      const c = document.createElement('canvas'); c.width = 960; c.height = 640;
      const ctx = c.getContext('2d'); ctx.fillStyle = '#eaf0f6'; ctx.fillRect(0, 0, 960, 640);
      if (!params.has('blank')) { ctx.fillStyle = '#16324f'; ctx.fillRect(100, 100, 400, 240); }
      return { dataUrl: c.toDataURL(), width: 960, height: 640, scaleFactor: 2, pointer: params.has('no-pointer') ? undefined : { x: 180, y: 160 },
        elementDetection: m.enabled, regions: params.has('blank') ? [] : [{ x: 50, y: 50, width: 800, height: 500, label: '窗口' }] };
    }
    if (command === 'capture_overlay_ready') {
      const image = document.querySelector('.capture-source');
      m.presentations.push({ ...(args.error ? { error: args.error } : {}), decoded: m.decoded, imageReady: !!image?.isConnected && image.complete && image.naturalWidth === 960, uiReady: !!document.querySelector('.capture-selection') && !document.querySelector('.capture-loading') });
      if (args.error) return;
      if (params.get('ready') === 'held') await new Promise(resolve => { m.releaseReady = resolve; });
      m.presented = true; return;
    }
    if (command === 'invalidate_capture_element') { m.invalidations.push(args.generation); return; }
    if (command === 'get_capture_element') {
      if (!m.presented) m.unexpected.push('query-before-ready');
      m.queries.push(args.point);
      m.generations.push(args.generation);
      if (m.hold) return new Promise(resolve => m.held.push({ resolve, point: args.point }));
      if (params.has('parents')) return [
        { x: args.point.x - 20, y: args.point.y - 20, width: 100, height: 70, label: '控件' },
        args.point.x < 400 ? { x: 100, y: 100, width: 400, height: 240, label: '父面板' } : { x: 500, y: 300, width: 300, height: 200, label: '新父面板' },
        { x: 50, y: 50, width: 800, height: 500, label: '窗口' },
      ];
      return { x: args.point.x - 20, y: args.point.y - 20, width: 100, height: 70, label: '控件' };
    }
    if (command === 'cancel_capture') return;
    m.unexpected.push(command);
    throw Error('Unexpected command ' + command);
  } };
} + ')()' });
const load = async extra => {
  await page.goto('http://127.0.0.1:1430/?view=capture' + extra);
  await wait(() => !!document.querySelector('.capture-selection'));
  await wait(() => !document.querySelector('.capture-detection-note')?.textContent.includes('正在识别'));
};
const selection = () => page.evaluate(() => {
  const s = document.querySelector('.capture-stage').__vueParentComponent.setupState;
  return JSON.parse(JSON.stringify({ hovered: s.hovered, pointer: s.pointer, selected: s.selected, error: s.error }));
});
const move = (x, y) => page.evaluate(async ({ x, y }) => {
  document.querySelector('.capture-stage').dispatchEvent(new PointerEvent('pointermove', { bubbles: true, clientX: x / 960 * innerWidth, clientY: y / 640 * innerHeight }));
  await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
}, { x, y });
try {
  await page.goto('http://127.0.0.1:1430/?view=capture&controls&decode=held&ready=held&no-raf');
  await wait(() => !!window._hoverMock?.releaseDecode);
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 120)));
  assert.deepEqual(await page.evaluate(() => ({ presentations: window._hoverMock.presentations.length, queries: window._hoverMock.queries.length, workers: window._hoverMock.workers })), { presentations: 0, queries: 0, workers: 0 });
  await page.evaluate(() => window._hoverMock.releaseDecode());
  await wait(() => !!window._hoverMock?.releaseReady);
  assert.deepEqual(await page.evaluate(() => window._hoverMock.presentations), [{ decoded: true, imageReady: true, uiReady: true }]);
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 120)));
  assert.deepEqual(await page.evaluate(() => ({ queries: window._hoverMock.queries.length, workers: window._hoverMock.workers })), { queries: 0, workers: 0 });
  await page.evaluate(() => window._hoverMock.releaseReady());
  await wait(() => window._hoverMock.queries.length > 0 && window._hoverMock.workers > 0);
  assert.deepEqual(await page.evaluate(() => window._hoverMock.unexpected), []);
  results.push({ name: 'native presentation waits for the displayed image decode and committed UI; detection waits for acknowledgement without relying on animation frames', pass: true });

  await page.goto('http://127.0.0.1:1430/?view=capture&controls&decode=error&no-raf');
  await wait(() => window._hoverMock?.presentations.length === 1);
  const failedStart = await page.evaluate(() => ({ calls: window._hoverMock.presentations, queries: window._hoverMock.queries.length, workers: window._hoverMock.workers, unexpected: window._hoverMock.unexpected }));
  assert.match(failedStart.calls[0].error, /fixture: displayed image decode failed/);
  assert.equal(failedStart.calls[0].decoded, false);
  assert.deepEqual({ queries: failedStart.queries, workers: failedStart.workers, unexpected: failedStart.unexpected }, { queries: 0, workers: 0, unexpected: [] });
  results.push({ name: 'displayed image decode failure reports initialization failure to native cleanup without revealing or starting detection', pass: true });

  await load('&blank');
  assert.equal((await selection()).pointer.x, 180);
  assert.equal((await selection()).pointer.y, 160);
  assert.equal((await selection()).hovered.width, 960);
  assert.equal(await page.evaluate(() => getComputedStyle(document.querySelector('.capture-selection')).outlineOffset), '-1px');
  await page.screenshot({ path: dir + 'fullscreen-inset-outline.png' });
  results.push({ name: 'startup uses native pointer instead of screen center; full-screen fallback has an inward visible outline', pass: true });
  for (const mode of ['constructor', 'async']) {
    await load('&worker=' + mode);
    assert.deepEqual((await selection()).hovered, { x: 100, y: 100, width: 400, height: 240, label: '边缘候选' });
    assert.equal((await selection()).error, '');
  }
  results.push({ name: 'worker startup and post-transfer failures still detect exact original-pixel rectangle through bounded fallback', pass: true });
  await page.goto('http://127.0.0.1:1430/?view=capture&worker=held');
  await wait(() => !!window._hoverMock?.releaseWorker);
  await key('Tab'); assert.equal((await selection()).hovered.width, 960);
  await page.evaluate(() => window._hoverMock.releaseWorker());
  assert.equal((await selection()).hovered.width, 960);
  await key('Shift'); await key('Tab', 'keydown', { shiftKey: true });
  const reverseChoice = (await selection()).hovered;
  await key('Shift', 'keyup');
  assert.deepEqual((await selection()).hovered, reverseChoice);
  assert.equal(reverseChoice.label, '窗口');
  results.push({ name: 'late worker results and releasing Shift after Shift+Tab preserve the manually chosen candidate', pass: true });
  await load('&controls');
  await wait(() => document.querySelector('.capture-dimensions').textContent.includes('控件'));
  assert.deepEqual((await selection()).hovered, { x: 160, y: 140, width: 100, height: 70, label: '控件' });
  await page.evaluate(() => { window._hoverMock.hold = true; });
  await move(260, 180); await wait(() => window._hoverMock.held.length === 1);
  await move(360, 220);
  await page.evaluate(() => window._hoverMock.held.shift().resolve({ x: 240, y: 160, width: 100, height: 70, label: '过期控件' }));
  await wait(() => window._hoverMock.held.length === 1);
  assert.notEqual((await selection()).hovered.label, '过期控件');
  await page.evaluate(() => window._hoverMock.held.shift().resolve({ x: 340, y: 200, width: 100, height: 70, label: '控件' }));
  await wait(() => document.querySelector('.capture-dimensions').textContent.includes('控件'));
  assert.equal((await selection()).hovered.x, 340);
  await move(390, 250); await wait(() => window._hoverMock.held.length === 1);
  await key('Tab'); const tabChoice = (await selection()).hovered;
  await page.evaluate(() => window._hoverMock.held.shift().resolve({ x: 370, y: 230, width: 100, height: 70, label: '控件' }));
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 100)));
  assert.deepEqual((await selection()).hovered, tabChoice);
  results.push({ name: 'native control hit wins over pixel edges, one request stays in flight, stale responses cannot overwrite moved pointer or Tab choice', pass: true });
  await key('Shift'); assert.equal((await selection()).hovered, undefined);
  await key('Shift', 'keyup');
  await page.evaluate(() => { window._hoverMock.hold = false; });
  await wait(() => document.querySelector('.capture-dimensions').textContent.includes('控件'));
  const chosen = (await selection()).hovered;
  await page.evaluate(() => {
    const stage = document.querySelector('.capture-stage'), target = document.querySelector('.capture-selection');
    stage.setPointerCapture = () => {}; stage.hasPointerCapture = () => false;
    const options = { bubbles: true, button: 0, pointerId: 17, clientX: 390 / 960 * innerWidth, clientY: 250 / 640 * innerHeight };
    target.dispatchEvent(new PointerEvent('pointerdown', options)); stage.dispatchEvent(new PointerEvent('pointerup', options));
  });
  const selected = (await selection()).selected;
  assert.deepEqual(selected, { x: chosen.x, y: chosen.y, width: chosen.width, height: chosen.height });
  const generations = await page.evaluate(() => ({ requests: window._hoverMock.generations, invalidations: window._hoverMock.invalidations, unexpected: window._hoverMock.unexpected }));
  assert.ok(generations.requests.every(g => Number.isSafeInteger(g) && generations.invalidations.includes(g)));
  assert.ok(generations.invalidations.at(-1) > generations.requests.at(-1));
  assert.deepEqual(generations.unexpected, []);
  results.push({ name: 'Shift temporarily disables prediction and a stationary click selects the actual control bounds', pass: true });

  await load('&controls&no-pointer');
  assert.equal(await page.evaluate(() => window._hoverMock.queries.length), 0);
  await move(180, 160);
  await wait(() => document.querySelector('.capture-dimensions').textContent.includes('控件'));
  assert.ok(await page.evaluate(() => window._hoverMock.queries.length > 0));
  results.push({ name: 'a monitor without the real startup pointer keeps fallback geometry and queries native controls only after pointer movement', pass: true });

  const loadParents = async () => {
    await page.goto('http://127.0.0.1:1430/?view=capture&controls&parents&worker=held');
    await wait(() => !!window._hoverMock?.releaseWorker && document.querySelector('.capture-dimensions')?.textContent.includes('控件'));
  };
  const reverse = async () => {
    await key('Shift'); await key('Tab', 'keydown', { shiftKey: true });
    const beforeRelease = (await selection()).hovered;
    await key('Shift', 'keyup');
    assert.deepEqual((await selection()).hovered, beforeRelease);
    return beforeRelease;
  };
  await loadParents();
  const initialControl = (await selection()).hovered;
  await key('Tab'); assert.equal((await selection()).hovered.label, '父面板');
  await page.evaluate(() => window._hoverMock.releaseWorker());
  await key('Tab'); assert.equal((await selection()).hovered.label, '窗口');
  await key('Tab'); const outer = (await selection()).hovered;
  assert.deepEqual(outer, { x: 0, y: 0, width: 960, height: 640, label: '整个画面' });
  for (let i = 0; i < 3; i++) { await key('Tab'); assert.deepEqual((await selection()).hovered, outer); }
  assert.equal((await reverse()).label, '窗口');
  assert.equal((await reverse()).label, '父面板');
  assert.deepEqual(await reverse(), initialControl);
  assert.deepEqual(await reverse(), initialControl);
  // A newly arrived pixel rectangle encloses the control but cannot alter this frozen path.
  assert.deepEqual(await page.evaluate(() => document.querySelector('.capture-stage').__vueParentComponent.setupState.keyboardChoices.map(r => r.label)), ['控件', '父面板', '窗口', '整个画面']);
  await move(600, 380);
  await wait(() => document.querySelector('.capture-stage').__vueParentComponent.setupState.hovered?.x === 580);
  await key('Tab'); assert.equal((await selection()).hovered.label, '新父面板');
  assert.equal((await selection()).hovered.x, 500);
  results.push({ name: 'Tab freezes a strictly enclosing path, stops at both ends, reverses despite late pixel candidates, and rebuilds after real movement', pass: true });

  await loadParents();
  await key('Tab'); await key('Tab'); assert.equal((await selection()).hovered.label, '窗口');
  await page.evaluate(() => {
    const stage = document.querySelector('.capture-stage');
    // Same image pixel: this older pointer event must not override the newer Shift state.
    stage.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, clientX: 180 / 960 * innerWidth, clientY: 160 / 640 * innerHeight, shiftKey: false }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Shift', bubbles: true }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true }));
    window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Shift', bubbles: true }));
  });
  assert.equal((await selection()).hovered.label, '父面板');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  assert.equal((await selection()).hovered.label, '父面板');
  assert.deepEqual(await page.evaluate(() => window._hoverMock.unexpected), []);
  results.push({ name: 'a queued same-pixel pointer event cannot erase the frozen path during Shift+Tab or after its animation frame', pass: true });
} finally { await page.cdp('Page.removeScriptToEvaluateOnNewDocument', { identifier: preload.identifier }); }
await fs.writeFile(dir + 'browser-results.json', JSON.stringify({ layer: 'Chromium UI and original pixels, simulated native geometry and errors; no real screen/AX access', results }, null, 2));
console.log({ results });
