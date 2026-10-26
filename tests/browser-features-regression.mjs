// Vite + an existing, authorized Ego TaskSpace/Page; never create/finish a space.
// ego-browser nodejs -e 'globalThis.starshotRegressionConfig={spaceId:26,page:"p2"}; await import("file:///ABSOLUTE_REPO/tests/browser-features-regression.mjs")'
// Uses original/generated images only. No desktop capture or clipboard access.
// Vue handlers receive DOM events; pointer capture alone is stubbed for synthetic pointers.
const fs = await import('node:fs/promises');
const { fileURLToPath } = await import('node:url');
const { join } = await import('node:path');
const assert = (await import('node:assert/strict')).default;
const config = globalThis.starshotRegressionConfig;
if (!Number.isInteger(config?.spaceId) || config.spaceId <= 0 || !config.page?.trim()) throw Error('Provide an existing authorized spaceId and page.');
const root = fileURLToPath(new URL('../', import.meta.url));
const artifacts = join(root, 'artifacts', 'feature-qa');
await fs.mkdir(artifacts, { recursive: true });
const page = (await taskSpace(config.spaceId)).page(config.page);
await page.goto('http://127.0.0.1:1430');
await page.waitForSelector('input[aria-label="导入本地图片"]', { state: 'attached' });
console.log(await page.snapshot());
const storage = await page.evaluate(() => Object.fromEntries(['starshot-theme', 'starshot-preferences'].map(key => [key, localStorage.getItem(key)])));
const results = [];
const record = (name, details = {}) => results.push({ name, pass: true, ...details });
const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const click = selector => page.evaluate(selector => { const element = document.querySelector(selector); if (!element || element.disabled) throw Error('Unavailable control: ' + selector); element.click(); }, selector);
const clickText = text => page.evaluate(text => { const matches = [...document.querySelectorAll('button')].filter(button => button.textContent.trim() === text && button.getClientRects().length); if (matches.length !== 1 || matches[0].disabled) throw Error('Ambiguous/unavailable button: ' + text); matches[0].click(); }, text);
const input = (label, value, event = 'input') => page.evaluate(({ label, value, event }) => { const element = document.querySelector(`[aria-label="${label}"]`); if (!element || element.disabled) throw Error('Unavailable input: ' + label); element.value = value; element.dispatchEvent(new Event(event, { bubbles: true })); }, { label, value, event });
const state = () => page.evaluate(() => {
  const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState;
  return JSON.parse(JSON.stringify({ annotations: s.annotations, style: s.style, crop: s.crop, selectedId: s.selectedId, appearance: s.appearance, presets: s.customPresets }));
});
const drag = (start, end = start) => page.evaluate(async ({ start, end }) => {
  const canvas = document.querySelector('canvas[aria-label="图片编辑画布"]'), s = canvas.__vueParentComponent.setupState;
  const rect = canvas.getBoundingClientRect(), l = s.layout;
  const event = (type, p, buttons) => new PointerEvent(type, { bubbles: true, pointerId: 991, pointerType: 'mouse', button: 0, buttons, clientX: rect.left + (p.x - l.crop.x + l.imageRect.x) * rect.width / l.width, clientY: rect.top + (p.y - l.crop.y + l.imageRect.y) * rect.height / l.height });
  const capture = canvas.setPointerCapture, hasCapture = canvas.hasPointerCapture;
  canvas.setPointerCapture = () => {}; canvas.hasPointerCapture = () => false;
  try {
    canvas.dispatchEvent(event('pointerdown', start, 1)); canvas.dispatchEvent(event('pointermove', end, 1));
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    canvas.dispatchEvent(event('pointerup', end, 0)); await Promise.resolve();
  }
  finally { canvas.setPointerCapture = capture; canvas.hasPointerCapture = hasCapture; }
}, { start, end });
const closeSettings = () => click('button[aria-label="关闭弹窗"]');

try {
  const engine = await page.evaluate(async () => {
    const { renderImage, DEFAULT_STYLE } = await import('/src/editor.ts');
    const source = document.createElement('canvas'); source.width = 64; source.height = 48;
    const sourceCtx = source.getContext('2d'); sourceCtx.fillStyle = '#f83a21'; sourceCtx.fillRect(0, 0, 64, 48);
    const sticker = document.createElement('canvas'); sticker.width = 64; sticker.height = 48;
    const stickerCtx = sticker.getContext('2d'); stickerCtx.fillStyle = '#00ff22'; stickerCtx.fillRect(0, 0, 64, 48);
    const base = { x: 12.2, y: 10.5, width: 20.1, height: 12.2, color: '#ffffff', lineWidth: 4 };
    const annotations = [{ ...base, id: 'mask-first', type: 'redact' }, { ...base, id: 'sticker-later', type: 'sticker', x: 0, y: 0, width: 64, height: 48, dataUrl: 'original-fixture' }, { ...base, id: 'mosaic-last', type: 'mosaic', x: 8, y: 8, width: 32, height: 24 }];
    const cases = [];
    for (const scale of [1, 2]) for (const cropped of [false, true]) {
      const crop = cropped ? { x: 7, y: 5, width: 40, height: 30 } : { x: 0, y: 0, width: 64, height: 48 };
      const output = renderImage(source, { width: 64, height: 48 }, annotations, { ...DEFAULT_STYLE, background: 'transparent', padding: 0, radius: 0, borderWidth: 0, shadow: 0 }, { crop, scale, stickers: new Map([['original-fixture', sticker]]) });
      const dataUrl = output.toDataURL('image/png'); const decoded = new Image(); decoded.src = dataUrl; await decoded.decode();
      const roundTrip = document.createElement('canvas'); roundTrip.width = output.width; roundTrip.height = output.height;
      roundTrip.getContext('2d').drawImage(decoded, 0, 0);
      let nonBlack = 0, checked = 0;
      for (const canvas of [output, roundTrip]) {
        const ctx = canvas.getContext('2d');
        for (let y = (10 - crop.y) * scale; y < (23 - crop.y) * scale; y++) for (let x = (12 - crop.x) * scale; x < (33 - crop.x) * scale; x++) {
          const p = ctx.getImageData(x, y, 1, 1).data; checked++;
          if (p[0] || p[1] || p[2] || p[3] !== 255) nonBlack++;
        }
      }
      cases.push({ scale, cropped, width: output.width, height: output.height, checked, nonBlack, dataUrl });
    }
    // The final-space repaint must retain rounded clipping instead of painting outside it.
    const cornerMask = [{ ...base, id: 'corner', type: 'redact', x: 0, y: 0, width: 16, height: 16 }];
    const rounded = renderImage(source, { width: 64, height: 48 }, cornerMask, { ...DEFAULT_STYLE, background: 'transparent', padding: 0, radius: 16, borderWidth: 0, shadow: 0 }, { scale: 2 });
    const cornerAlpha = rounded.getContext('2d').getImageData(0, 0, 1, 1).data[3];
    return { cases, cornerAlpha, originalPixel: [...sourceCtx.getImageData(15, 12, 1, 1).data] };
  });
  assert.equal(engine.cornerAlpha, 0); assert.deepEqual(engine.originalPixel, [248, 58, 33, 255]);
  for (const result of engine.cases) {
    assert.equal(result.nonBlack, 0, `redaction ${result.scale}x crop=${result.cropped}`);
    await fs.writeFile(join(artifacts, `redact-${result.scale}x-${result.cropped ? 'crop' : 'full'}.png`), Buffer.from(result.dataUrl.split(',')[1], 'base64'));
    delete result.dataUrl;
  }
  record('opaque redaction survives later sticker/mosaic, crop, 1x/2x PNG and rounded clipping', engine);

  await clickText('用一张原创示例，试试星镜');
  await page.waitForSelector('canvas[aria-label="图片编辑画布"]', { state: 'visible' });
  await click('button[aria-label="矩形"]'); await drag({ x: 100, y: 100 }, { x: 250, y: 200 });
  let current = await state(); assert.equal(current.annotations.length, 1);
  const rectangle = current.annotations[0];
  await click('button[aria-label="选择对象"]'); await drag({ x: 100, y: 100 }, { x: 140, y: 130 });
  current = await state(); assert.ok(Math.abs(current.annotations[0].x - rectangle.x - 40) < .001); assert.ok(Math.abs(current.annotations[0].y - rectangle.y - 30) < .001);
  await input('选中对象颜色', '#2266bb'); assert.equal((await state()).annotations[0].color, '#2266bb');
  await clickText('删除选中对象'); assert.equal((await state()).annotations.length, 0);
  await click('button[aria-label="撤销"]'); assert.equal((await state()).annotations.length, 1);
  record('select, move, recolor, delete and undo an existing rectangle');

  await click('button[aria-label="文字"]'); await input('标注文字', '原创文字'); await drag({ x: 320, y: 280 });
  current = await state(); const textId = current.annotations.at(-1).id;
  await input('选择标注对象', textId, 'change'); await input('编辑选中文字', '修改后的原创文字', 'change');
  assert.equal((await state()).annotations.at(-1).text, '修改后的原创文字');
  await click('button[aria-label="撤销"]'); assert.equal((await state()).annotations.at(-1).text, '原创文字');
  record('edit selected text and undo without recreating its object');

  const sticker = await page.evaluate(() => { const c = document.createElement('canvas'); c.width = 160; c.height = 80; const x = c.getContext('2d'); x.fillStyle = '#38ba84'; x.fillRect(0, 0, 110, 80); x.fillStyle = '#f4b833'; x.beginPath(); x.arc(120, 40, 32, 0, 2 * Math.PI); x.fill(); return c.toDataURL('image/png'); });
  const stickerPath = join(artifacts, 'original-sticker.png'); await fs.writeFile(stickerPath, Buffer.from(sticker.split(',')[1], 'base64'));
  await page.setInputFiles('input[aria-label="导入本地贴纸"]', [stickerPath]);
  await page.waitForFunction(() => document.querySelector('input[aria-label="贴纸旋转"]'));
  current = await state(); const before = current.annotations.at(-1); assert.equal(before.type, 'sticker');
  await input('贴纸旋转', '90'); assert.equal((await state()).annotations.at(-1).rotation, 90);
  await clickText('放大贴纸'); assert.ok(Math.abs((await state()).annotations.at(-1).width - before.width * 1.1) < .001);
  await click('button[aria-label="撤销"]'); assert.equal((await state()).annotations.at(-1).width, before.width);
  await click('button[aria-label="撤销"]'); assert.equal((await state()).annotations.at(-1).rotation, 0);
  await clickText('删除选中对象'); assert.equal((await state()).annotations.length, 2);
  await click('button[aria-label="撤销"]'); assert.equal((await state()).annotations.at(-1).type, 'sticker');
  await settle(); await page.screenshot({ path: join(artifacts, 'object-editor.png') });
  record('local transparent sticker import, rotate, scale, delete and undo');

  await click('button[aria-label="实心遮盖"]');
  const beforeMasks = (await state()).annotations.length;
  for (const end of [{ x: 100, y: 100 }, { x: 200, y: 100 }, { x: 100, y: 200 }]) {
    await drag({ x: 100, y: 100 }, end);
    assert.equal((await state()).annotations.length, beforeMasks);
    assert.equal(await page.evaluate(() => document.querySelector('[role="status"]').textContent.includes('尺寸必须大于零')), false);
  }
  await drag({ x: 180, y: 180 }, { x: 260, y: 230 });
  assert.equal((await state()).annotations.at(-1).type, 'redact');
  const savedMask = await page.evaluate(async () => {
    const { parseSession } = await import('/src/session.ts');
    const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState;
    const session = parseSession(await s.getSession());
    const annotations = session.documents.find(d => d.id === session.currentId).state.annotations;
    return { schemaVersion: session.schemaVersion, mask: annotations.at(-1) };
  });
  assert.equal(savedMask.schemaVersion, 2); assert.equal(savedMask.mask.type, 'redact');
  assert.ok(savedMask.mask.width > 2 && savedMask.mask.height > 2);
  record('zero-area manual redactions are ignored and a real mask serializes as schema 2');

  await clickText('图片美化'); await input('渐变起始颜色', '#123456'); await input('渐变结束颜色', '#aabbcc'); await input('渐变角度', '45', 'change'); await input('四周留白', '48');
  await input('美化预设名称', 'QA原创渐变'); await click('.preset-save button');
  await click('button[aria-label="极光漫游"]'); await clickText('QA原创渐变');
  current = await state(); assert.equal(current.style.gradientFrom, '#123456'); assert.equal(current.style.gradientTo, '#aabbcc'); assert.equal(current.style.padding, 48); assert.equal(current.style.gradientAngle, 45);
  await click('button[aria-label="设置与帮助"]');
  await page.evaluate(() => { const checkbox = [...document.querySelectorAll('input[type="checkbox"]')].find(input => input.parentElement.textContent.includes('启用自己的配色')); if (!checkbox.checked) checkbox.click(); });
  await input('界面起始颜色', '#de8eb2'); await input('界面结束颜色', '#5285b8'); await input('界面渐变方向', '210');
  await closeSettings();
  assert.equal((await state()).style.gradientFrom, '#123456');
  await page.reload(); await page.waitForSelector('input[aria-label="导入本地图片"]', { state: 'attached' });
  await clickText('用一张原创示例，试试星镜'); await page.waitForSelector('canvas[aria-label="图片编辑画布"]', { state: 'visible' });
  await clickText('QA原创渐变'); current = await state();
  assert.equal(current.style.gradientAngle, 45); assert.equal(current.style.padding, 48); assert.equal(current.appearance.enabled, true); assert.equal(current.appearance.from, '#de8eb2'); assert.equal(current.appearance.angle, 210);
  assert.equal(await page.evaluate(() => document.documentElement.style.getPropertyValue('--accent-solid')), '#de8eb2');
  assert.equal(await page.evaluate(() => localStorage.getItem('starshot-preferences').includes('data:image/')), false);
  record('custom style and separate interface colors survive reload without persisting image pixels');

  await click('button[aria-label="设置与帮助"]'); await clickText('预览产品图');
  await page.waitForFunction(() => document.querySelector('.share-preview')?.complete);
  const share = await page.evaluate(() => { const img = document.querySelector('.share-preview'); return { width: img.naturalWidth, height: img.naturalHeight, dataUrl: img.src }; });
  const shareBytes = Buffer.from(share.dataUrl.split(',')[1], 'base64');
  assert.equal(share.width, 1080); assert.equal(share.height, 1440); assert.ok(shareBytes.length < 512 * 1024);
  await fs.writeFile(join(artifacts, 'share-generated.png'), shareBytes);
  const downloadPromise = page.waitForEvent('download', { timeout: 30000 });
  await clickText('保存产品图'); const download = await downloadPromise;
  await download.saveAs(join(artifacts, 'share-downloaded.png'));
  assert.deepEqual(await fs.readFile(join(artifacts, 'share-downloaded.png')), shareBytes);
  record('original share art preview and real browser download', { width: share.width, height: share.height, bytes: shareBytes.length });
  await page.screenshot({ path: join(artifacts, 'share-settings.png') });

  // Exercise the global decoded-pixel budget through genuine import/history handlers.
  await page.reload(); await page.waitForSelector('input[aria-label="导入本地图片"]', { state: 'attached' });
  await clickText('用一张原创示例，试试星镜'); await page.waitForSelector('canvas[aria-label="图片编辑画布"]', { state: 'visible' });
  const budgetSticker = await page.evaluate(() => { const c = document.createElement('canvas'); c.width = 1000; c.height = 2000; const x = c.getContext('2d'); x.fillStyle = '#2672a6'; x.fillRect(0, 0, c.width, c.height); return c.toDataURL('image/png'); });
  const budgetPath = join(artifacts, 'original-2mp-sticker.png'); await fs.writeFile(budgetPath, Buffer.from(budgetSticker.split(',')[1], 'base64'));
  const budgetState = () => page.evaluate(() => {
    const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState;
    return { id: s.currentId, annotations: s.annotations.length, undo: s.undoStack.length, redo: s.redoStack.length };
  });
  const importBudgetSticker = async expected => {
    await page.setInputFiles('input[aria-label="导入本地贴纸"]', [budgetPath]);
    await page.waitForFunction(expected => document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState.annotations.length === expected, expected);
  };
  const openRecent = async (index, id) => {
    await click('button[aria-label="最近图片"]');
    await page.evaluate(index => document.querySelectorAll('.history-grid button')[index].click(), index);
    await page.waitForFunction(id => document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState.currentId === id, id);
  };
  for (let count = 1; count <= 4; count++) await importBudgetSticker(count);
  const docA = (await budgetState()).id;
  await click('button[aria-label="撤销"]'); assert.equal((await budgetState()).annotations, 3);
  await page.setInputFiles('input[aria-label="导入本地图片"]', [stickerPath]);
  await page.waitForFunction(id => document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState.currentId !== id, docA);
  const docB = (await budgetState()).id;
  await importBudgetSticker(1);
  await openRecent(1, docA);
  const blockedRedo = await budgetState(); await click('button[aria-label="重做"]'); assert.deepEqual(await budgetState(), blockedRedo);
  assert.equal(await page.evaluate(() => document.querySelector('[role="status"]').textContent.includes('800 万像素')), true);
  await openRecent(0, docB); await click('button[aria-label="撤销"]');
  await openRecent(1, docA); await click('button[aria-label="重做"]'); assert.equal((await budgetState()).annotations, 4);
  // Select the last sticker through the public object list before deleting it.
  const lastStickerId = (await state()).annotations.at(-1).id;
  await input('选择标注对象', lastStickerId, 'change'); await clickText('删除选中对象');
  assert.equal((await budgetState()).annotations, 3);
  await openRecent(0, docB); await click('button[aria-label="重做"]'); assert.equal((await budgetState()).annotations, 1);
  await openRecent(1, docA); const blockedUndo = await budgetState();
  await click('button[aria-label="撤销"]'); assert.deepEqual(await budgetState(), blockedUndo);
  const budgetSession = await page.evaluate(async () => {
    const { parseSession } = await import('/src/session.ts');
    const s = document.querySelector('canvas[aria-label="图片编辑画布"]').__vueParentComponent.setupState;
    const session = parseSession(await s.getSession()); return { schemaVersion: session.schemaVersion, counts: session.documents.map(d => d.state.annotations.length) };
  });
  assert.deepEqual(budgetSession, { schemaVersion: 2, counts: [1, 3] });
  record('cross-document sticker budget blocks unsafe undo/redo, preserves stacks, and remains serializable');
  console.log({ results, artifacts });
} catch (error) {
  results.push({ name: 'failure', pass: false, message: String(error), stack: error.stack });
  console.log(await page.snapshot());
  throw error;
} finally {
  await fs.writeFile(join(artifacts, 'results.json'), JSON.stringify({ layer: 'real Chromium Canvas and Vue UI with synthetic DOM pointer events; no native clipboard or desktop access', results }, null, 2));
  await page.evaluate(storage => { for (const [key, value] of Object.entries(storage)) if (value === null) localStorage.removeItem(key); else localStorage.setItem(key, value); }, storage);
}
// Parent owns TaskSpace lifecycle. Keep this page until its final review is complete.
