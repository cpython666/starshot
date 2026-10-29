// Vite + an existing authorized Ego TaskSpace/Page; this script never creates/finishes a space.
// ego-browser nodejs -e 'globalThis.starshotRegressionConfig={spaceId:SPACE_ID,page:"PAGE_LABEL"}; await import("file:///ABSOLUTE_REPO/tests/browser-details-regression.mjs")'
// Generated pixels, real Vue handlers/PNG rendering, and mocked native settings/pin handoff.
// No desktop capture, clipboard access, real shortcut registration, or native permission changes.
const fs = await import('node:fs/promises');
const { fileURLToPath } = await import('node:url');
const { join } = await import('node:path');
const assert = (await import('node:assert/strict')).default;
const config = globalThis.starshotRegressionConfig;
if (!Number.isInteger(config?.spaceId) || config.spaceId <= 0 || typeof config.page !== 'string' || !config.page.trim()) throw Error('Pass an existing authorized {spaceId,page}.');
const root = fileURLToPath(new URL('../', import.meta.url));
const artifacts = join(root, 'artifacts', 'starclip-details-20261029');
await fs.mkdir(artifacts, { recursive: true });
const version = JSON.parse(await fs.readFile(join(root, 'package.json'), 'utf8')).version;
const page = (await taskSpace(config.spaceId)).page(config.page), results = [];
const source = '(' + function (version) {
  window.isTauri = true;
  const callbacks = new Map(), listeners = new Map(); let id = 0;
  const mock = window._detailsMock = {
    calls: [], unexpected: [], state: null, failNextSettings: false, failureActual: null, holdSettingsGet: false, holdSettingsSave: false,
    pendingPinEdit: null, pinClaims: [], pinReceipts: [],
    settings: { captureShortcut: 'CommandOrControl+Shift+7', pinShortcut: 'CommandOrControl+Alt+8', startInTray: false, autostart: false, pinHalo: true },
  };
  const clone = value => JSON.parse(JSON.stringify(value));
  mock.emit = (event, payload = null) => {
    for (const [eventId, listener] of listeners) if (listener.event === event) callbacks.get(listener.handler)({ event, id: eventId, payload });
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener(_event, id) { listeners.delete(id); } };
  window.__TAURI_INTERNALS__ = {
    transformCallback(callback) { callbacks.set(++id, callback); return id; },
    async invoke(command, args = {}) {
      mock.calls.push({ command, args: clone(args) });
      if (command === 'plugin:event|listen') { listeners.set(++id, args); return id; }
      if (command === 'plugin:event|unlisten') { listeners.delete(args.eventId); return; }
      if (command === 'get_current_image') return null;
      if (command === 'take_pending_pin_edit') {
        const pending = mock.pendingPinEdit; mock.pendingPinEdit = null;
        if (pending) mock.pinClaims.push(pending.requestId);
        return pending ? clone(pending) : null;
      }
      if (command === 'finish_pin_edit') {
        const s = document.querySelector('canvas[aria-label="图片编辑画布"]')?.__vueParentComponent.setupState;
        mock.pinReceipts.push({ ...clone(args), imageAtFinish: s?.image?.src, documentIdAtFinish: s?.currentId });
        return;
      }
      if (command === 'get_app_status') return { platform: 'macos', captureAvailable: true, elementDetection: true };
      if (command === 'get_desktop_settings') {
        const snapshot = clone(mock.settings);
        if (mock.holdSettingsGet) { mock.holdSettingsGet = false; await new Promise(resolve => { mock.finishSettingsGet = resolve; }); }
        return snapshot;
      }
      if (command === 'set_desktop_settings') {
        if (mock.failNextSettings) {
          mock.failNextSettings = false;
          if (mock.failureActual) mock.settings = clone(mock.failureActual);
          throw Error('原创测试：快捷键保存失败，请核对实际生效绑定');
        }
        mock.settings = clone(args.settings);
        const snapshot = clone(mock.settings);
        if (mock.holdSettingsSave) { mock.holdSettingsSave = false; await new Promise(resolve => { mock.finishSettingsSave = resolve; }); }
        return snapshot;
      }
      if (command === 'set_editor_state') { mock.state = clone(args); return; }
      if (command === 'get_update_status') return { revision: 0, currentVersion: version, configured: true, phase: 'idle', downloadReady: false, confirmationPending: false, downloaded: 0 };
      if (command === 'list_recovery_points') return [];
      if (command === 'get_recovery_storage') return { recoveryBytes: 0, applicationBytes: 0, totalBytes: 0 };
      mock.unexpected.push(command); throw Error('Unexpected native operation: ' + command);
    },
  };
} + ')(' + JSON.stringify(version) + ')';
const preload = await page.cdp('Page.addScriptToEvaluateOnNewDocument', { source });
const wait = condition => page.waitForFunction(condition, undefined, { timeout: 15_000 });
const click = selector => page.evaluate(selector => {
  const button = document.querySelector(selector);
  if (!button || button.matches(':disabled')) throw Error('Unavailable control: ' + selector);
  button.click();
}, selector);
const clickText = text => page.evaluate(text => {
  const buttons = [...document.querySelectorAll('button')].filter(button => button.textContent.trim() === text && button.getClientRects().length);
  if (buttons.length !== 1 || buttons[0].matches(':disabled')) throw Error('Ambiguous/unavailable button: ' + text);
  buttons[0].click();
}, text);
const input = (label, value) => page.evaluate(({ label, value }) => {
  const element = document.querySelector(`[aria-label="${label}"]`);
  if (!element || element.matches(':disabled')) throw Error('Unavailable input: ' + label);
  element.value = value; element.dispatchEvent(new Event('input', { bubbles: true }));
}, { label, value });
const readImage = () => page.evaluate(async () => {
  const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState;
  const png = new Image(); png.src = s.output('png'); await png.decode();
  const canvas = document.createElement('canvas'); canvas.width = png.width; canvas.height = png.height;
  const ctx = canvas.getContext('2d'); ctx.drawImage(png, 0, 0);
  const digest = async bytes => [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(byte => byte.toString(16).padStart(2, '0')).join('');
  const sample = (x, y) => [...ctx.getImageData((s.layout.imageRect.x + x - s.crop.x) * s.outputScale, (s.layout.imageRect.y + y - s.crop.y) * s.outputScale, 1, 1).data];
  return JSON.parse(JSON.stringify({
    id: s.currentId, sourceHash: await digest(new TextEncoder().encode(s.image.src)), style: s.style, crop: s.crop, annotations: s.annotations,
    backgroundHash: s.backgroundImage ? await digest(new TextEncoder().encode(s.backgroundImage.src)) : null,
    outputHash: await digest(ctx.getImageData(0, 0, canvas.width, canvas.height).data), width: png.width, height: png.height, scale: s.outputScale,
    samples: { left: sample(30, 70), right: sample(100, 70), mask: sample(50, 50), background: [...ctx.getImageData(5, 5, 1, 1).data] },
    undo: s.undoStack.length, redo: s.redoStack.length, hasCustomStyle: s.hasCustomStyle, status: s.status,
  }));
});
const preserveContent = (actual, expected) => {
  for (const key of ['id', 'sourceHash', 'crop', 'annotations']) assert.deepEqual(actual[key], expected[key], key + ' must survive style-only changes');
  assert.deepEqual(actual.samples.left, [241, 75, 50, 255]);
  assert.deepEqual(actual.samples.right, [40, 120, 220, 255]);
  assert.deepEqual(actual.samples.mask, [0, 0, 0, 255]);
};
const readShortcuts = () => page.evaluate(() => ({
  mac: navigator.platform.includes('Mac'), header: document.querySelector('.header-actions kbd')?.textContent.trim(),
  help: [...document.querySelectorAll('.shortcut-list > div')].slice(0, 2).map(row => row.querySelector('kbd').textContent.trim()),
  capture: document.querySelector('[aria-label="开始截图快捷键"]').value, pin: document.querySelector('[aria-label="贴图快捷键"]').value,
  message: document.querySelector('[aria-label="开始截图快捷键"]').closest('.settings-section').querySelector('[role="status"]')?.textContent || '',
}))
const assertShortcuts = (actual, capture, pin) => {
  const label = value => value.replace(/CommandOrControl|CmdOrCtrl/g, actual.mac ? '⌘' : 'Ctrl').replaceAll('+', ' + ');
  assert.equal(actual.header, label(capture)); assert.deepEqual(actual.help, [label(capture), label(pin)]);
  assert.equal(actual.capture, capture); assert.equal(actual.pin, pin);
};
const readEditor = () => page.evaluate(() => {
  const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState;
  return JSON.parse(JSON.stringify({ id: s.currentId, source: s.image.src, name: s.currentName, size: s.size,
    documents: s.documents.map(doc => ({ id: doc.id, name: doc.name, dataUrl: doc.dataUrl })),
    crop: s.crop || null, annotations: s.annotations, style: s.style, undo: s.undoStack.length, redo: s.redoStack.length, output: s.output('png') }));
});
const queuePinEdit = (requestId, fixture, options = {}) => page.evaluate(({ requestId, fixture, options }) => {
  const mock = window._detailsMock;
  mock.pendingPinEdit = { requestId, expiresAt: Date.now() + (options.lifetime ?? 15_000), image: { dataUrl: mock.pinFixtures[fixture], name: `原创贴图-${requestId}.png`, width: 96, height: 64 } };
  mock.pinExpiresAt = mock.pendingPinEdit.expiresAt;
  mock.emit('pin-edit-requested');
  if (options.repeat) mock.emit('pin-edit-requested');
}, { requestId, fixture, options });
const pinReceipt = requestId => page.evaluate(requestId => window._detailsMock.pinReceipts.filter(receipt => receipt.requestId === requestId), requestId);
const waitForPinReceipt = async requestId => {
  await page.waitForFunction(requestId => window._detailsMock.pinReceipts.some(receipt => receipt.requestId === requestId), requestId, { timeout: 15_000 });
  await wait(() => !document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState.ioPending);
  const receipts = await pinReceipt(requestId); assert.equal(receipts.length, 1, 'each pin edit receives exactly one completion'); return receipts[0];
};
const delayPinDecode = fixture => page.evaluate(fixture => {
  const mock = window._detailsMock, original = HTMLImageElement.prototype.decode;
  mock.releasePinDecode = null;
  mock.restoreDecode = () => { HTMLImageElement.prototype.decode = original; };
  HTMLImageElement.prototype.decode = function () {
    if (this.src !== mock.pinFixtures[fixture]) return original.call(this);
    return new Promise((resolve, reject) => { mock.releasePinDecode = () => original.call(this).then(resolve, reject); });
  };
}, fixture);

try {
  await page.goto('http://127.0.0.1:1430');
  await wait(() => window._detailsMock?.state && document.querySelector('[aria-label="开始截图快捷键"]'));
  console.log(await page.snapshot());
  const fixtures = await page.evaluate(() => {
    const source = document.createElement('canvas'); source.width = 160; source.height = 120;
    const ctx = source.getContext('2d'); ctx.fillStyle = '#f14b32'; ctx.fillRect(0, 0, 80, 120); ctx.fillStyle = '#2878dc'; ctx.fillRect(80, 0, 80, 120);
    const background = document.createElement('canvas'); background.width = 64; background.height = 64;
    const bg = background.getContext('2d'); bg.fillStyle = '#23a370'; bg.fillRect(0, 0, 64, 64);
    return { 'original-source.png': source.toDataURL('image/png'), 'original-background.png': background.toDataURL('image/png'), 'slow-background.png': background.toDataURL('image/png') };
  });
  for (const [name, dataUrl] of Object.entries(fixtures)) await fs.writeFile(join(artifacts, name), Buffer.from(dataUrl.split(',')[1], 'base64'));
  await page.setInputFiles('input[aria-label="导入本地图片"]', [join(artifacts, 'original-source.png')]);
  await wait(() => document.querySelector('canvas[aria-label="图片编辑画布"]') && window._detailsMock.state?.hasSession && !window._detailsMock.state.busy);
  const defaultStyle = await page.evaluate(async () => {
    const { DEFAULT_STYLE } = await import('/src/editor.ts');
    const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState;
    s.checkpoint(); s.crop = { x: 20, y: 20, width: 100, height: 80 };
    s.annotations = [{ id: 'original-mask', type: 'redact', x: 40, y: 40, width: 20, height: 20, color: '#000000', lineWidth: 4 }];
    for (const [key, value] of Object.entries({ padding: 24, radius: 0, borderWidth: 0, shadow: 0, aspect: '1:1', gradientFrom: '#e29911' })) s.setStyle(key, value);
    return DEFAULT_STYLE;
  });
  await page.setInputFiles('input[aria-label="导入背景图片"]', [join(artifacts, 'original-background.png')]);
  await wait(() => { const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState; return s.backgroundImage && !s.ioPending; });
  const customized = await readImage();
  assert.equal(customized.style.background, 'image'); assert.deepEqual(customized.samples.background, [35, 163, 112, 255]);
  await clickText('恢复默认美化');
  const reset = await readImage();
  preserveContent(reset, customized); assert.deepEqual(reset.style, defaultStyle); assert.equal(reset.backgroundHash, null);
  assert.equal(reset.undo, customized.undo + 1); assert.equal(reset.redo, 0); assert.equal(reset.hasCustomStyle, false);
  assert.equal(reset.width, (100 + 2 * defaultStyle.padding) * reset.scale); assert.equal(reset.height, (80 + 2 * defaultStyle.padding) * reset.scale);
  assert.notEqual(reset.outputHash, customized.outputHash);
  assert.equal(await page.evaluate(() => [...document.querySelectorAll('button')].find(button => button.textContent.trim() === '恢复默认美化').disabled), true);
  results.push({ name: 'restore default beauty preserves crop, source pixels and opaque annotation; clears local background in one undo step', pass: true, customizedPixels: customized.outputHash, defaultPixels: reset.outputHash, samples: reset.samples });

  await click('button[aria-label="撤销"]'); const undone = await readImage();
  preserveContent(undone, customized); assert.deepEqual(undone.style, customized.style);
  assert.equal(undone.backgroundHash, customized.backgroundHash); assert.equal(undone.outputHash, customized.outputHash);
  assert.equal(undone.undo, customized.undo); assert.equal(undone.redo, 1);
  await click('button[aria-label="重做"]'); const redone = await readImage();
  preserveContent(redone, reset); assert.deepEqual(redone.style, defaultStyle); assert.equal(redone.backgroundHash, null);
  assert.equal(redone.outputHash, reset.outputHash); assert.equal(redone.undo, reset.undo); assert.equal(redone.redo, 0);
  results.push({ name: 'undo restores custom style and background with identical full PNG pixels; redo exactly restores default output', pass: true });

  await page.evaluate(() => {
    const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState; s.setStyle('padding', 48);
    const NativeFileReader = window.FileReader;
    window._detailsMock.restoreReader = () => { window.FileReader = NativeFileReader; };
    window.FileReader = class extends NativeFileReader {
      readAsDataURL(file) {
        if (file.name !== 'slow-background.png') return super.readAsDataURL(file);
        window._detailsMock.releaseBackground = () => super.readAsDataURL(file);
      }
    };
  });
  await page.setInputFiles('input[aria-label="导入背景图片"]', [join(artifacts, 'slow-background.png')]);
  await wait(() => !!window._detailsMock.releaseBackground);
  await clickText('恢复默认美化'); const resetWhileLoading = await readImage();
  await page.evaluate(() => window._detailsMock.releaseBackground());
  await wait(() => !document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState.ioPending);
  const late = await readImage(); preserveContent(late, resetWhileLoading);
  for (const key of ['style', 'backgroundHash', 'outputHash', 'undo', 'redo', 'status']) assert.deepEqual(late[key], resetWhileLoading[key], 'late background must not change ' + key);
  await page.evaluate(() => window._detailsMock.restoreReader());
  results.push({ name: 'reset invalidates a pending background read; late completion cannot change pixels, status or undo history', pass: true });

  await click('button[aria-label="设置与帮助"]');
  let shortcuts = await readShortcuts(); assertShortcuts(shortcuts, 'CommandOrControl+Shift+7', 'CommandOrControl+Alt+8');
  await input('开始截图快捷键', 'CommandOrControl+Shift+6'); await input('贴图快捷键', 'CommandOrControl+Alt+9');
  await clickText('保存桌面设置');
  await wait(() => document.querySelector('[aria-label="开始截图快捷键"]').closest('.settings-section').querySelector('[role="status"]')?.textContent.includes('已保存并生效'));
  shortcuts = await readShortcuts(); assertShortcuts(shortcuts, 'CommandOrControl+Shift+6', 'CommandOrControl+Alt+9');
  assert.deepEqual(await page.evaluate(() => window._detailsMock.calls.filter(call => call.command === 'set_desktop_settings').at(-1).args.settings), { captureShortcut: 'CommandOrControl+Shift+6', pinShortcut: 'CommandOrControl+Alt+9', startInTray: false, autostart: false, pinHalo: true });
  results.push({ name: 'native initial settings and successful save update both header and help shortcut labels', pass: true, header: shortcuts.header, help: shortcuts.help });

  const readsBeforeFailure = await page.evaluate(() => {
    // Simulate a native fallback binding that differs from both the old UI value and rejected draft.
    window._detailsMock.failNextSettings = true;
    window._detailsMock.failureActual = { ...window._detailsMock.settings, captureShortcut: 'CommandOrControl+Shift+5', pinShortcut: 'CommandOrControl+Alt+4' };
    return window._detailsMock.calls.filter(call => call.command === 'get_desktop_settings').length;
  });
  await input('开始截图快捷键', 'CommandOrControl+Shift+0'); await input('贴图快捷键', 'CommandOrControl+Alt+0');
  await clickText('保存桌面设置');
  await wait(() => document.querySelector('[aria-label="开始截图快捷键"]').closest('.settings-section').querySelector('[role="status"]')?.textContent.includes('原创测试：快捷键保存失败'));
  shortcuts = await readShortcuts(); assertShortcuts(shortcuts, 'CommandOrControl+Shift+5', 'CommandOrControl+Alt+4');
  assert.equal(await page.evaluate(() => window._detailsMock.calls.filter(call => call.command === 'get_desktop_settings').length), readsBeforeFailure + 1);
  assert.match(shortcuts.message, /原创测试：快捷键保存失败/);
  results.push({ name: 'failed save re-reads actual native bindings into inputs, header and help while retaining the failure message', pass: true, header: shortcuts.header, help: shortcuts.help });

  assert.equal(await page.evaluate(() => document.querySelector('[aria-label="贴图焦点光圈"]').checked), true);
  await input('开始截图快捷键', 'CommandOrControl+Shift+3'); await input('贴图快捷键', 'CommandOrControl+Alt+1');
  await page.evaluate(() => { window._detailsMock.settings.pinHalo = false; window._detailsMock.emit('pin-halo', false); });
  await wait(() => !document.querySelector('[aria-label="贴图焦点光圈"]').checked);
  const draftsAfterHalo = await readShortcuts();
  assert.equal(draftsAfterHalo.capture, 'CommandOrControl+Shift+3'); assert.equal(draftsAfterHalo.pin, 'CommandOrControl+Alt+1');
  assert.equal(draftsAfterHalo.header, shortcuts.header); assert.deepEqual(draftsAfterHalo.help, shortcuts.help);
  await input('开始截图快捷键', shortcuts.capture); await input('贴图快捷键', shortcuts.pin);
  results.push({ name: 'shared halo changes update only the checkbox and preserve unsaved shortcut inputs and actual-binding labels', pass: true });

  await page.evaluate(() => {
    const mock = window._detailsMock;
    mock.holdSettingsGet = true; mock.settingsRefreshDone = false;
    const settings = document.querySelector('[aria-label="开始截图快捷键"]').closest('.settings-section').__vueParentComponent.setupState;
    void settings.refresh().then(() => { mock.settingsRefreshDone = true; });
  });
  await wait(() => typeof window._detailsMock.finishSettingsGet === 'function');
  await page.evaluate(() => { window._detailsMock.settings.pinHalo = true; window._detailsMock.emit('pin-halo', true); window._detailsMock.finishSettingsGet(); });
  await wait(() => window._detailsMock.settingsRefreshDone);
  assert.equal(await page.evaluate(() => document.querySelector('[aria-label="贴图焦点光圈"]').checked), true);
  assertShortcuts(await readShortcuts(), shortcuts.capture, shortcuts.pin);

  await input('开始截图快捷键', 'CommandOrControl+Shift+6');
  await page.evaluate(() => { window._detailsMock.holdSettingsSave = true; });
  await clickText('保存桌面设置');
  await wait(() => typeof window._detailsMock.finishSettingsSave === 'function');
  await page.evaluate(() => { window._detailsMock.settings.pinHalo = false; window._detailsMock.emit('pin-halo', false); window._detailsMock.finishSettingsSave(); });
  await wait(() => !document.querySelector('[aria-label="开始截图快捷键"]').matches(':disabled'));
  assert.equal(await page.evaluate(() => document.querySelector('[aria-label="贴图焦点光圈"]').checked), false);
  assertShortcuts(await readShortcuts(), 'CommandOrControl+Shift+6', shortcuts.pin);
  assert.equal(await page.evaluate(() => window._detailsMock.calls.filter(call => call.command === 'set_desktop_settings').at(-1).args.settings.pinHalo), true);
  results.push({ name: 'settings get/save responses cannot overwrite a newer shared halo event; saved shortcuts still reach inputs and labels', pass: true });

  const help = await page.evaluate(() => document.querySelector('.dialog').textContent);
  assert.match(help, /手动逐屏长截图/); assert.doesNotMatch(help, /滚动长截图与系统控件级识别尚未提供/);
  assert.deepEqual(await page.evaluate(() => window._detailsMock.unexpected), []);
  results.push({ name: 'about text describes implemented scrolling capture; no unexpected native operations occurred', pass: true });

  await click('button[aria-label="关闭弹窗"]');
  const pinFixtures = await page.evaluate(() => {
    const fixtures = { invalid: 'data:image/png;base64,YnJva2VuLW9yaWdpbmFsLWZpeHR1cmU=' };
    for (const [name, color] of Object.entries({ delivered: '#8b49d2', delayed: '#397cc5', replacement: '#dd9438' })) {
      const canvas = document.createElement('canvas'); canvas.width = 96; canvas.height = 64;
      const context = canvas.getContext('2d'); context.fillStyle = color; context.fillRect(0, 0, 96, 64);
      context.fillStyle = '#ebf5bb'; context.fillRect(12, 12, 24, 20); fixtures[name] = canvas.toDataURL('image/png');
    }
    window._detailsMock.pinFixtures = fixtures; return fixtures;
  });
  const beforeDelivery = await readEditor();
  await queuePinEdit(101, 'delivered', { repeat: true });
  const deliveredReceipt = await waitForPinReceipt(101), delivered = await readEditor();
  assert.equal(deliveredReceipt.error, null); assert.equal(deliveredReceipt.imageAtFinish, pinFixtures.delivered);
  assert.equal(deliveredReceipt.documentIdAtFinish, delivered.id); assert.equal(delivered.source, pinFixtures.delivered);
  assert.notEqual(delivered.id, beforeDelivery.id); assert.equal(delivered.documents.length, beforeDelivery.documents.length + 1);
  assert.deepEqual(delivered.size, { width: 96, height: 64 });
  assert.deepEqual(await page.evaluate(() => window._detailsMock.pinClaims.filter(id => id === 101)), [101]);
  results.push({ name: 'pin edit wakeup claims a request once and confirms success only after decoded pixels enter the current document', pass: true });

  const beforeBusy = await readEditor();
  await page.evaluate(() => { document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState.privacyBusy = true; });
  await queuePinEdit(102, 'replacement'); const busyReceipt = await waitForPinReceipt(102);
  assert.ok(typeof busyReceipt.error === 'string' && busyReceipt.error.length > 0, 'busy editor must explicitly reject');
  assert.deepEqual(await readEditor(), beforeBusy);
  await page.evaluate(() => { document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState.privacyBusy = false; });
  results.push({ name: 'busy editor rejects pin handoff with an error and preserves its complete editing session', pass: true });

  const beforeInvalid = await readEditor();
  await queuePinEdit(103, 'invalid'); const invalidReceipt = await waitForPinReceipt(103);
  assert.ok(typeof invalidReceipt.error === 'string' && invalidReceipt.error.length > 0, 'decode failure must never acknowledge success');
  assert.deepEqual(await readEditor(), beforeInvalid);
  results.push({ name: 'invalid pin pixels return an explicit decode failure without importing a document', pass: true });

  const beforeExpiry = await readEditor();
  await delayPinDecode('delayed'); await queuePinEdit(104, 'delayed', { lifetime: 1000 });
  await wait(() => !!window._detailsMock.releasePinDecode);
  assert.deepEqual(await pinReceipt(104), [], 'a pending decode cannot be confirmed');
  await wait(() => Date.now() >= window._detailsMock.pinExpiresAt);
  await page.evaluate(() => window._detailsMock.releasePinDecode());
  const expiredReceipt = await waitForPinReceipt(104);
  assert.ok(typeof expiredReceipt.error === 'string' && expiredReceipt.error.length > 0, 'expired request must explicitly reject');
  assert.deepEqual(await readEditor(), beforeExpiry);
  await page.evaluate(() => window._detailsMock.restoreDecode());
  results.push({ name: 'pin decode completing after request expiry cannot import pixels, alter history, or report success', pass: true });

  await delayPinDecode('delayed'); await queuePinEdit(105, 'delayed');
  await wait(() => !!window._detailsMock.releasePinDecode);
  await page.evaluate(async () => {
    const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState;
    await s.loadImage({ dataUrl: window._detailsMock.pinFixtures.replacement, name: '主动选择的新原创图.png' });
  });
  const newer = await readEditor(); assert.equal(newer.source, pinFixtures.replacement);
  await page.evaluate(() => window._detailsMock.releasePinDecode());
  const replacedReceipt = await waitForPinReceipt(105);
  assert.ok(typeof replacedReceipt.error === 'string' && replacedReceipt.error.length > 0, 'superseded request must explicitly reject');
  assert.deepEqual(await readEditor(), newer);
  await page.evaluate(() => window._detailsMock.restoreDecode());
  assert.deepEqual(await page.evaluate(() => window._detailsMock.unexpected), []);
  results.push({ name: 'a newer user import cancels pending pin delivery; late decode cannot replace the newer image or append history', pass: true });
} catch (error) {
  results.push({ name: 'failure', pass: false, message: String(error), stack: error.stack });
  console.log(await page.snapshot()); throw error;
} finally {
  await fs.writeFile(join(artifacts, 'browser-results.json'), JSON.stringify({ version, layer: 'Chromium Vue UI, generated PNG pixels and mocked IPC; no native shortcut or desktop verification', results }, null, 2));
  await page.evaluate(() => { window._detailsMock?.restoreReader?.(); window._detailsMock?.restoreDecode?.(); });
  await page.cdp('Page.removeScriptToEvaluateOnNewDocument', { identifier: preload.identifier });
}
console.log({ results, artifact: join(artifacts, 'browser-results.json') });
// The parent owns this TaskSpace and its lifecycle.
