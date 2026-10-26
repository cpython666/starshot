// Run with the Vite server and an already authorized, active Ego task space:
//   ego-browser nodejs -e 'globalThis.starshotRegressionConfig={spaceId:SPACE_ID,page:"PAGE_LABEL"}; await import("file://REPO_PATH/tests/browser-import-regression.mjs")'
// This uses only generated fixtures. It never reads the desktop or clipboard.
// It resumes the configured space and page; it never creates or finishes a space.
const fs = await import('node:fs/promises');
const { fileURLToPath } = await import('node:url');
const { join } = await import('node:path');
const assert = (await import('node:assert/strict')).default;
const config = globalThis.starshotRegressionConfig;
if (!Number.isInteger(config?.spaceId) || config.spaceId <= 0 || typeof config.page !== 'string' || !config.page.trim()) {
  throw new Error('Set globalThis.starshotRegressionConfig={spaceId, page} to an already authorized, active Ego task space and its existing page before importing this script.');
}
const root = fileURLToPath(new URL('../', import.meta.url));
const artifacts = join(root, 'artifacts', 'import-regression');
await fs.mkdir(artifacts, { recursive: true });
const task = await taskSpace(config.spaceId);
const page = task.page(config.page);
await page.goto('http://127.0.0.1:1430');
await page.waitForSelector('input[aria-label="导入本地图片"]', { state: 'attached' });
console.log(await page.snapshot());
const fixtures = await page.evaluate(() => {
  const colors = { 'slow-first.png': '#ef5566', 'last-selected.png': '#3979d9', 'next-document.png': '#2a9e6a', 'slow-background.png': '#e9cb52', 'slow-preset.png': '#aa68d9' };
  return Object.entries(colors).map(([name, color]) => {
    const canvas = document.createElement('canvas');
    canvas.width = 96; canvas.height = 64;
    const ctx = canvas.getContext('2d');
    ctx.fillStyle = color; ctx.fillRect(0, 0, 96, 64);
    ctx.fillStyle = '#fff'; ctx.fillRect(10, 10, 15, 15);
    return { name, dataUrl: canvas.toDataURL('image/png') };
  });
});
for (const fixture of fixtures) await fs.writeFile(`${artifacts}/${fixture.name}`, Buffer.from(fixture.dataUrl.split(',')[1], 'base64'));
await fs.writeFile(`${artifacts}/slow-broken.png`, 'original deliberately invalid PNG fixture');
await page.evaluate(() => {
  const NativeFileReader = window.FileReader;
  const gates = new Map();
  const completed = [];
  window.__starshotRegression = { gates, completed, restore() { window.FileReader = NativeFileReader; } };
  window.FileReader = class extends NativeFileReader {
    readAsDataURL(file) {
      if (!file.name.startsWith('slow-')) return super.readAsDataURL(file);
      gates.set(file.name, () => {
        const completion = new Promise(resolve => this.addEventListener('loadend', () => { completed.push(file.name); resolve(); }, { once: true }));
        super.readAsDataURL(file);
        return completion;
      });
    }
  };
});
const readState = () => page.evaluate(() => {
  const canvas = document.querySelector('canvas[aria-label="图片编辑画布"]');
  let hash = 0;
  if (canvas) for (const byte of canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data) hash = ((hash << 5) - hash + byte) | 0;
  return {
    title: document.querySelector('.document-title')?.textContent,
    background: document.querySelector('.segmented .active')?.textContent,
    preset: document.querySelector('.gradient-swatch[aria-pressed="true"]')?.getAttribute('aria-label'),
    status: document.querySelector('[role="status"]')?.textContent,
    modal: document.querySelector('[role="dialog"]')?.getAttribute('aria-label') || null,
    hash,
  };
});
const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const release = async name => {
  await page.evaluate(async name => {
    const gate = window.__starshotRegression.gates.get(name);
    if (!gate) throw new Error(`Missing delayed read: ${name}`);
    await gate();
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  }, name);
};
const importSource = name => page.setInputFiles('input[aria-label="导入本地图片"]', [`${artifacts}/${name}`]);
const importBackground = name => page.setInputFiles('input[aria-label="导入背景图片"]', [`${artifacts}/${name}`]);
const results = [];
try {
  await importSource('slow-first.png');
  await importSource('last-selected.png');
  await page.waitForFunction(() => document.querySelector('.document-title')?.textContent.includes('last-selected.png'));
  await settle();
  const latest = await readState();
  await release('slow-first.png');
  const afterSlow = await readState();
  assert.equal(afterSlow.title, latest.title);
  assert.equal(afterSlow.hash, latest.hash);
  results.push({ name: 'slow earlier source cannot replace last selected source', pass: true });

  await importBackground('slow-background.png');
  await importSource('next-document.png');
  await page.waitForFunction(() => document.querySelector('.document-title')?.textContent.includes('next-document.png'));
  await settle();
  const next = await readState();
  await release('slow-background.png');
  const afterBackground = await readState();
  assert.equal(afterBackground.title, next.title);
  assert.equal(afterBackground.background, '渐变');
  assert.equal(afterBackground.hash, next.hash);
  results.push({ name: 'background read cannot cross into another document', pass: true });

  await importBackground('slow-preset.png');
  await page.click('button[aria-label="极光漫游"]');
  await settle();
  const preset = await readState();
  await release('slow-preset.png');
  const afterPreset = await readState();
  assert.equal(afterPreset.preset, '极光漫游');
  assert.equal(afterPreset.background, '渐变');
  assert.equal(afterPreset.hash, preset.hash);
  results.push({ name: 'explicit gradient selection supersedes pending background', pass: true });

  await importSource('slow-broken.png');
  await importSource('last-selected.png');
  await page.waitForFunction(() => document.querySelector('.document-title')?.textContent.includes('last-selected.png'));
  await settle();
  const valid = await readState();
  await release('slow-broken.png');
  const afterBroken = await readState();
  assert.equal(afterBroken.title, valid.title);
  assert.equal(afterBroken.status, valid.status);
  assert.equal(afterBroken.hash, valid.hash);
  results.push({ name: 'stale invalid source cannot replace latest success status', pass: true });

  await page.click('button[aria-label="极光漫游"]');
  await settle();
  const edited = await readState();
  await page.click('button[aria-label="设置与帮助"]');
  await page.waitForSelector('[role="dialog"]', { state: 'visible' });
  await page.keyboard.press('ControlOrMeta+z');
  await settle();
  const modalState = await readState();
  assert.equal(modalState.modal, '设置与帮助');
  assert.equal(modalState.hash, edited.hash);
  const shortcuts = await page.evaluate(() => {
    let downloads = 0;
    const original = HTMLAnchorElement.prototype.click;
    HTMLAnchorElement.prototype.click = function () { downloads++; };
    try {
      const target = document.querySelector('.dialog button');
      const save = new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true, cancelable: true });
      const open = new KeyboardEvent('keydown', { key: 'o', metaKey: true, bubbles: true, cancelable: true });
      target.dispatchEvent(save); target.dispatchEvent(open);
      return { downloads, savePrevented: save.defaultPrevented, openPrevented: open.defaultPrevented };
    } finally { HTMLAnchorElement.prototype.click = original; }
  });
  assert.deepEqual(shortcuts, { downloads: 0, savePrevented: true, openPrevented: true });
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => !document.querySelector('[role="dialog"]'));
  await page.click('button[aria-label="撤销"]');
  await settle();
  assert.notEqual((await readState()).hash, edited.hash);
  results.push({ name: 'modal blocks editor undo/save/open and normal undo still works afterward', pass: true });

  await fs.writeFile(`${artifacts}/results.json`, JSON.stringify({ date: '2026-10-26', layer: 'real Chromium UI with controlled FileReader completion', results }, null, 2));
  console.log({ results, artifact: `${artifacts}/results.json` });
} finally {
  await page.evaluate(() => window.__starshotRegression?.restore());
}
console.log(await page.snapshot());
// The parent owns the shared TaskSpace lifecycle; do not finish it here.
