// Existing authorized Ego TaskSpace only; generated pixels and mocked native IPC.
// Verifies UI routing, not OS topmost windows or native focus/drag/double-click delivery.
const fs = await import('node:fs/promises');
const assert = (await import('node:assert/strict')).default;
const { fileURLToPath } = await import('node:url');
const config = globalThis.starshotRegressionConfig;
if (!Number.isInteger(config?.spaceId) || !config.page) throw Error('Pass an existing authorized {spaceId,page}');
const page = (await taskSpace(config.spaceId)).page(config.page), results = [];
const dir = fileURLToPath(new URL('../artifacts/pin-interactions/', import.meta.url));
await fs.mkdir(dir, { recursive: true });
const source = '(' + function () {
  const callbacks = new Map(), listeners = new Map();
  let callbackId = 0, listenerId = 0;
  const fixture = JSON.parse(localStorage.getItem('_starshotPinHaloFixture') || '{}');
  const mock = window._pinMock = { calls: [], failClose: false, failExport: false, cancelSave: false, holdExport: false, failHalo: false, halo: fixture.halo ?? true };
  mock.emit = (event, payload, target = { kind: 'Any' }) => {
    for (const [id, listener] of listeners) {
      if (listener.event === event && (target.kind === 'Any' || listener.target.kind === 'Any' || (listener.target.kind === target.kind && listener.target.label === target.label))) callbacks.get(listener.handler)?.({ event, id, payload });
    }
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener(_event, id) { listeners.delete(id); } };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'pin-fixture' } },
    transformCallback(callback) { const id = ++callbackId; callbacks.set(id, callback); return id; },
    async invoke(command, args = {}) {
      mock.calls.push({ command, args: JSON.parse(JSON.stringify(args)) });
      if (command === 'get_pin_image') {
        const canvas = document.createElement('canvas'); canvas.width = 400; canvas.height = 300;
        const ctx = canvas.getContext('2d'); ctx.fillStyle = '#182d4a'; ctx.fillRect(0, 0, 400, 300);
        ctx.fillStyle = '#dff5a4'; ctx.fillRect(60, 50, 140, 100);
        const snapshot = { dataUrl: canvas.toDataURL('image/png'), width: 400, height: 300, pinHalo: mock.halo, focused: fixture.focused ?? true, gutter: 12 };
        if (fixture.holdImage) await new Promise(resolve => { mock.finishImage = resolve; });
        return snapshot;
      }
      if (command === 'pin_action') {
        if (args.action === 'close' && mock.failClose) throw Error('fixture: close failed');
        if (args.action === 'halo') {
          if (mock.failHalo) throw Error('fixture: halo save failed');
          mock.halo = args.value === 1;
          localStorage.setItem('_starshotPinHaloFixture', JSON.stringify({ halo: mock.halo }));
          mock.emit('pin-halo', mock.halo);
        }
        return;
      }
      if (command === 'pin_export') {
        if (mock.failExport) throw Error('fixture: export failed');
        if (mock.holdExport) await new Promise(resolve => { mock.finishExport = resolve; });
        return { completed: args.action !== 'save' || !mock.cancelSave, path: args.action === 'save' && !mock.cancelSave ? '/fixture/星镜贴图.png' : null };
      }
      if (command === 'plugin:event|listen') {
        const id = ++listenerId; listeners.set(id, args);
        if (args.event === 'tauri://focus' && fixture.focusOnListen) { mock.focusOnListenEmitted = true; mock.emit('tauri://focus', null, { kind: 'Window', label: 'pin-fixture' }); }
        if (args.event === 'pin-halo' && typeof fixture.haloOnListen === 'boolean') mock.emit('pin-halo', fixture.haloOnListen);
        return id;
      }
      if (command === 'plugin:event|unlisten') return;
      throw Error('Unexpected native command: ' + command);
    },
  };
} + ')()';
const preload = await page.cdp('Page.addScriptToEvaluateOnNewDocument', { source });
const wait = fn => page.waitForFunction(fn, undefined, { timeout: 10_000 });
const load = async () => {
  await page.goto('http://127.0.0.1:1430/?view=pin');
  await wait(() => document.querySelector('.pin-image-area img')?.complete);
};
const key = (key, extra = {}) => page.evaluate(({ key, extra }) => window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...extra })), { key, extra });
const openMenu = async () => {
  await page.evaluate(() => document.querySelector('.pin-image-area').dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: innerWidth - 2, clientY: innerHeight - 2 })));
  await wait(() => document.querySelector('[role=menu]')?.contains(document.activeElement));
};
const actions = () => page.evaluate(() => window._pinMock.calls.filter(c => c.command === 'pin_action').map(c => c.args));
try {
  await load();
  await page.evaluate(() => localStorage.removeItem('_starshotPinHaloFixture'));
  await load();
  await page.evaluate(() => document.querySelector('.pin-image-area').dispatchEvent(new PointerEvent('pointerdown', { button: 2, buttons: 2, bubbles: true })));
  await openMenu();
  assert.deepEqual(await actions(), []);
  const bounds = await page.evaluate(() => { const r = document.querySelector('[role=menu]').getBoundingClientRect(); return { x: r.x, y: r.y, right: r.right, bottom: r.bottom, width: innerWidth, height: innerHeight }; });
  assert.ok(bounds.x >= 8 && bounds.y >= 8 && bounds.right <= bounds.width - 7 && bounds.bottom <= bounds.height - 7, JSON.stringify(bounds));
  await key('End'); assert.match(await page.evaluate(() => document.activeElement.textContent), /关闭贴图/);
  await key('ArrowDown'); assert.match(await page.evaluate(() => document.activeElement.textContent), /复制图片/);
  await key('Escape'); assert.equal(await page.evaluate(() => !!document.querySelector('[role=menu]')), false);
  assert.deepEqual(await actions(), []);
  await key('F10', { shiftKey: true }); await wait(() => !!document.querySelector('[role=menu]'));
  await page.evaluate(() => document.querySelector('.pin-image-area').dispatchEvent(new PointerEvent('pointerdown', { button: 0, buttons: 1, bubbles: true })));
  assert.equal(await page.evaluate(() => !!document.querySelector('[role=menu]')), false);
  assert.deepEqual(await actions(), []);
  results.push({ name: 'right-click and keyboard open a bounded menu; arrow keys wrap; Esc and outside click dismiss without closing or dragging', pass: true });

  await openMenu();
  await page.evaluate(() => document.querySelector('[role=menu]').dispatchEvent(new WheelEvent('wheel', { deltaY: -100, bubbles: true, cancelable: true })));
  assert.deepEqual(await actions(), []);
  const clickItem = text => page.evaluate(text => { const button = [...document.querySelectorAll('[role=menuitem]')].find(el => el.querySelector('span').textContent === text); if (!button) throw Error('Missing menu item: ' + text); button.click(); }, text);
  await clickItem('向右旋转 90°');
  assert.match(await page.evaluate(() => document.querySelector('.pin-image-area img').style.transform), /rotate\(90deg\)/);
  await openMenu(); await clickItem('水平翻转');
  assert.match(await page.evaluate(() => document.querySelector('.pin-image-area img').style.transform), /scaleX\(-1\)/);
  await openMenu(); await clickItem('降低不透明度');
  assert.deepEqual((await actions()).at(-1), { action: 'opacity', value: .9 });
  await openMenu(); await clickItem('隐藏工具栏');
  assert.equal(await page.evaluate(() => document.querySelector('.pin-toolbar').style.display), 'none');
  await openMenu(); await clickItem('恢复大小、方向与透明度');
  await wait(() => window._pinMock.calls.some(c => c.args.action === 'scale'));
  assert.match(await page.evaluate(() => document.querySelector('.pin-image-area img').style.transform), /rotate\(0deg\).*scaleX\(1\)/);
  assert.ok((await actions()).some(a => a.action === 'scale' && a.value === 1));
  await openMenu(); await clickItem('开启鼠标穿透');
  assert.deepEqual((await actions()).at(-1), { action: 'ignore-cursor', value: 1 });
  results.push({ name: 'menu reuses rotation, flip, opacity, toolbar, reset and cursor passthrough; menu scrolling never zooms the image', pass: true });

  await page.cdp('Emulation.setDeviceMetricsOverride', { width: 180, height: 130, deviceScaleFactor: 1, mobile: false });
  await wait(() => document.querySelector('.pin-window').__vueParentComponent.setupState.viewport.width === 180);
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await openMenu();
  const small = await page.evaluate(() => { const r = document.querySelector('[role=menu]').getBoundingClientRect(); return { x: r.x, y: r.y, right: r.right, bottom: r.bottom }; });
  assert.ok(small.x >= 8 && small.y >= 8 && small.right <= 173 && small.bottom <= 123, JSON.stringify(small));
  await key('End');
  assert.equal(await page.evaluate(() => { const item = document.activeElement.getBoundingClientRect(), menu = document.querySelector('[role=menu]').getBoundingClientRect(); return item.bottom <= menu.bottom && item.top >= menu.top; }), true);
  await page.screenshot({ path: dir + 'small-menu.png' });
  await page.cdp('Emulation.clearDeviceMetricsOverride');
  results.push({ name: 'small pin viewport keeps the menu on-screen and keyboard navigation scrolls the last item into view', pass: true });

  await load();
  await page.evaluate(() => {
    const area = document.querySelector('.pin-image-area');
    const press = (x, y, ctrlKey = false) => area.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0, buttons: 1, screenX: x, screenY: y, ctrlKey }));
    press(100, 100, true); // macOS Control-click belongs to the menu.
    press(100, 100); press(150, 150); // Same client position after moving the window is not a double click.
    area.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, buttons: 1, screenX: 200, screenY: 200 }));
    press(150, 150); // A completed drag also cancels the earlier click candidate.
    press(151, 151); // Native drag may have consumed click/up; the second press still closes.
    area.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
  });
  assert.deepEqual((await actions()).map(a => a.action), ['drag', 'drag', 'drag', 'close']);
  results.push({ name: 'native-drag fallback closes once on same-position consecutive presses, excludes Control-click and real movement, and deduplicates DOM dblclick', pass: true });

  await load();
  await page.evaluate(() => { window._pinMock.failClose = true; document.querySelector('.pin-image-area').dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); });
  await wait(() => document.querySelector('[role=alert]')?.textContent.includes('fixture: close failed'));
  await page.evaluate(() => { window._pinMock.failClose = false; });
  await key('Escape');
  assert.equal((await actions()).filter(a => a.action === 'close').length, 2);
  results.push({ name: 'close failure reports an error and permits retry with Escape', pass: true });

  await load();
  await key('r'); await key('f'); await key('+');
  await page.evaluate(() => document.querySelector('.pin-window').dispatchEvent(new WheelEvent('wheel', { deltaY: 100, ctrlKey: true, bubbles: true, cancelable: true })));
  await key('c', { metaKey: true });
  await wait(() => document.querySelector('[role=status]')?.textContent.includes('图片已复制'));
  const exports = () => page.evaluate(() => window._pinMock.calls.filter(c => c.command === 'pin_export').map(c => c.args));
  assert.deepEqual((await exports()).at(-1), { action: 'copy', rotation: 90, flipped: true });
  assert.equal(await page.evaluate(() => document.querySelector('.pin-scale').textContent), '110%');
  assert.equal(await page.evaluate(() => document.querySelector('.pin-image-area img').style.opacity), '0.95');
  await openMenu();
  assert.match(await page.evaluate(() => document.querySelector('.pin-export-note').textContent), /缩放与透明度只影响桌面预览/);
  await clickItem('复制图片');
  await wait(() => window._pinMock.calls.filter(c => c.command === 'pin_export').length === 2);
  results.push({ name: 'copy menu and Cmd+C pass only current rotation/flip to the owner-scoped command; zoom and opacity remain preview-only', pass: true });

  await page.evaluate(() => { window._pinMock.cancelSave = true; });
  await key('s', { ctrlKey: true });
  await wait(() => document.querySelector('[role=status]')?.textContent.includes('已取消保存，贴图保留'));
  assert.equal((await actions()).filter(a => a.action === 'close').length, 0);
  await page.evaluate(() => { window._pinMock.cancelSave = false; window._pinMock.failExport = true; });
  await key('s', { ctrlKey: true });
  await wait(() => document.querySelector('[role=alert]')?.textContent.includes('fixture: export failed'));
  assert.match(await page.evaluate(() => document.querySelector('.pin-image-area img').style.transform), /rotate\(90deg\).*scaleX\(-1\)/);
  await page.evaluate(() => { window._pinMock.failExport = false; });
  await openMenu(); await clickItem('另存为…');
  await wait(() => document.querySelector('[role=status]')?.textContent.includes('已保存到 /fixture/星镜贴图.png'));
  assert.equal(await page.evaluate(() => !!document.querySelector('[role=alert]')), false);
  results.push({ name: 'save cancellation, failure and retry keep the pin and orientation; success reports the actual returned destination', pass: true });

  await page.evaluate(() => { window._pinMock.holdExport = true; });
  await key('c', { ctrlKey: true });
  await wait(() => !!window._pinMock.finishExport);
  const exportCount = (await exports()).length, actionCount = (await actions()).length;
  await key('c', { ctrlKey: true }); await key('s', { ctrlKey: true }); await key('Escape'); await key('r'); await key('f');
  await page.evaluate(() => {
    const area = document.querySelector('.pin-image-area');
    area.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0, buttons: 1 }));
    area.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    document.querySelector('button[aria-label="向右旋转图片"]').click();
  });
  assert.equal((await exports()).length, exportCount);
  assert.equal((await actions()).length, actionCount);
  assert.match(await page.evaluate(() => document.querySelector('.pin-image-area img').style.transform), /rotate\(90deg\).*scaleX\(-1\)/);
  await page.evaluate(() => { window._pinMock.holdExport = false; window._pinMock.finishExport(); });
  await wait(() => !document.querySelector('.pin-window').__vueParentComponent.setupState.busy);
  await key('e', { ctrlKey: true });
  await wait(() => document.querySelector('[role=status]')?.textContent.includes('已发送到编辑器，贴图仍保留'));
  assert.deepEqual((await exports()).at(-1), { action: 'edit', rotation: 90, flipped: true });
  await openMenu(); await clickItem('送到编辑器');
  await wait(() => window._pinMock.calls.filter(c => c.command === 'pin_export' && c.args.action === 'edit').length === 2);
  assert.equal((await actions()).filter(a => a.action === 'close').length, 0);
  results.push({ name: 'an in-flight export blocks duplicates, close/drag and orientation changes; edit menu and Ctrl+E preserve the pin', pass: true });

  await load();
  assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-halo')), true);
  assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-focused')), true);
  const focusTargets = await page.evaluate(() => window._pinMock.calls.filter(c => c.command === 'plugin:event|listen' && ['tauri://focus', 'tauri://blur'].includes(c.args.event)).map(c => ({ event: c.args.event, target: c.args.target })));
  assert.deepEqual(focusTargets, [
    { event: 'tauri://focus', target: { kind: 'Window', label: 'pin-fixture' } },
    { event: 'tauri://blur', target: { kind: 'Window', label: 'pin-fixture' } },
  ]);
  const activeBorder = await page.evaluate(() => getComputedStyle(document.querySelector('.pin-frame'), '::after').borderTopColor);
  await page.evaluate(() => window._pinMock.emit('tauri://blur', null, { kind: 'Window', label: 'pin-other' }));
  assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-focused')), true);
  await page.evaluate(() => window._pinMock.emit('tauri://blur', null, { kind: 'Window', label: 'pin-fixture' }));
  await wait(() => !document.querySelector('.pin-window').classList.contains('pin-focused'));
  await page.waitForFunction(color => getComputedStyle(document.querySelector('.pin-frame'), '::after').borderTopColor !== color, activeBorder, { timeout: 10_000 });
  await page.evaluate(() => window._pinMock.emit('tauri://focus', null, { kind: 'Window', label: 'pin-fixture' }));
  await wait(() => document.querySelector('.pin-window').classList.contains('pin-focused'));
  results.push({ name: 'halo defaults on with native initial focus; only this pin receives focus/blur and the active border differs from inactive', pass: true });

  const imageGeometry = () => page.evaluate(() => { const img = document.querySelector('.pin-image-area img'), r = img.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, transform: img.style.transform }; });
  const beforeHaloOff = await imageGeometry();
  await openMenu(); await clickItem('关闭贴图光圈');
  await wait(() => !document.querySelector('.pin-window').classList.contains('pin-halo'));
  assert.deepEqual((await actions()).at(-1), { action: 'halo', value: 0 });
  assert.deepEqual(await imageGeometry(), beforeHaloOff);
  await key('c', { ctrlKey: true });
  await wait(() => document.querySelector('[role=status]')?.textContent.includes('图片已复制'));
  assert.deepEqual((await exports()).at(-1), { action: 'copy', rotation: 0, flipped: false });
  await load();
  assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-halo')), false);
  await page.evaluate(() => { window._pinMock.failHalo = true; });
  await openMenu(); await clickItem('开启贴图光圈');
  await wait(() => document.querySelector('[role=alert]')?.textContent.includes('fixture: halo save failed'));
  assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-halo')), false);
  assert.deepEqual(await imageGeometry(), beforeHaloOff);
  await page.evaluate(() => { window._pinMock.failHalo = false; });
  await openMenu(); await clickItem('开启贴图光圈');
  await wait(() => document.querySelector('.pin-window').classList.contains('pin-halo'));
  await page.evaluate(() => window._pinMock.emit('pin-halo', false));
  await wait(() => !document.querySelector('.pin-window').classList.contains('pin-halo'));
  await page.evaluate(() => window._pinMock.emit('pin-halo', true));
  await wait(() => document.querySelector('.pin-window').classList.contains('pin-halo'));
  results.push({ name: 'halo menu persists the choice across reload, keeps image geometry and export clean, retains old state on failure, and follows shared settings events', pass: true });

  await load();
  await page.cdp('Emulation.setDeviceMetricsOverride', { width: 180, height: 130, deviceScaleFactor: 1, mobile: false });
  await wait(() => document.querySelector('.pin-window').__vueParentComponent.setupState.viewport.width === 180);
  for (let turn = 0; turn < 4; turn++) {
    if (turn) await key('r');
    const geometry = await page.evaluate(() => {
      const frame = document.querySelector('.pin-frame').getBoundingClientRect(), img = document.querySelector('.pin-image-area img').getBoundingClientRect();
      return { frame: { x: frame.x, y: frame.y, right: frame.right, bottom: frame.bottom, width: frame.width, height: frame.height }, img: { x: img.x, y: img.y, width: img.width, height: img.height } };
    });
    const { frame, img } = geometry;
    assert.ok(frame.x >= 11.9 && frame.y >= 11.9 && frame.right <= 168.1 && frame.bottom <= 118.1, JSON.stringify(geometry));
    for (const axis of ['x', 'y', 'width', 'height']) assert.ok(Math.abs(frame[axis] - img[axis]) < .1, `${axis}: ${JSON.stringify(geometry)}`);
  }
  await key(' ');
  await page.screenshot({ path: dir + 'halo-active.png' });
  await page.evaluate(() => window._pinMock.emit('tauri://blur', null, { kind: 'Window', label: 'pin-fixture' }));
  await wait(() => !document.querySelector('.pin-window').classList.contains('pin-focused'));
  await page.screenshot({ path: dir + 'halo-inactive.png' });
  results.push({ name: 'small viewport reserves a full halo gutter and frame follows the actual image bounds at all four rotations', pass: true });

  await page.cdp('Emulation.clearDeviceMetricsOverride');
  await page.evaluate(() => localStorage.setItem('_starshotPinHaloFixture', JSON.stringify({ focused: false, focusOnListen: true, halo: true, haloOnListen: false })));
  await load();
  assert.equal(await page.evaluate(() => window._pinMock.focusOnListenEmitted), true);
  assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-focused')), false);
  assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-halo')), true);
  results.push({ name: 'initial native snapshot supersedes focus/halo events received while listeners were still being registered', pass: true });

  for (const focused of [false, true]) {
    await page.evaluate(focused => localStorage.setItem('_starshotPinHaloFixture', JSON.stringify({ focused, halo: focused, holdImage: true })), focused);
    await page.goto('http://127.0.0.1:1430/?view=pin');
    await wait(() => typeof window._pinMock.finishImage === 'function');
    await page.evaluate(focused => {
      window._pinMock.emit(focused ? 'tauri://blur' : 'tauri://focus', null, { kind: 'Window', label: 'pin-fixture' });
      window._pinMock.emit('pin-halo', !focused);
      window._pinMock.finishImage();
    }, focused);
    await wait(() => document.querySelector('.pin-image-area img')?.complete);
    assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-focused')), !focused);
    assert.equal(await page.evaluate(() => document.querySelector('.pin-window').classList.contains('pin-halo')), !focused);
  }
  results.push({ name: 'focus, blur and halo events arriving during the initial image request survive its stale response in both directions', pass: true });
} finally {
  await page.evaluate(() => localStorage.removeItem('_starshotPinHaloFixture'));
  await page.cdp('Emulation.clearDeviceMetricsOverride');
  await page.cdp('Page.removeScriptToEvaluateOnNewDocument', { identifier: preload.identifier });
}
await fs.writeFile(dir + 'results.json', JSON.stringify({ layer: 'Chromium Vue UI and mocked native IPC; no OS screenshot, clipboard, topmost, focus or drag verification', results }, null, 2));
console.log({ results });
