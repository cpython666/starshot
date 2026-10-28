// Run inside the parent's existing authorized Ego TaskSpace; synthetic pixels only.
// globalThis.starshotRegressionConfig={spaceId:ID,page:'pN'}; await import('file:///ABS_REPO/tests/browser-preview-regression.mjs')
const fs = await import('node:fs/promises');
const { fileURLToPath } = await import('node:url');
const { join } = await import('node:path');
const assert = (await import('node:assert/strict')).default;
const config = globalThis.starshotRegressionConfig;
if (!Number.isInteger(config?.spaceId) || !config.page) throw Error('Provide an existing authorized spaceId and page');
const page = (await taskSpace(config.spaceId)).page(config.page);
await page.goto('http://127.0.0.1:1430');
await page.waitForSelector('input[aria-label="导入本地图片"]', { state: 'attached' });
const evidence = await page.evaluate(async () => {
  const { renderPreview, renderImage, DEFAULT_STYLE } = await import('/src/editor.ts');
  const source = document.createElement('canvas'); source.width = 320; source.height = 240;
  const ctx = source.getContext('2d'); ctx.fillStyle = '#f83a21'; ctx.fillRect(0, 0, 320, 240);
  const sticker = document.createElement('canvas'); sticker.width = 20; sticker.height = 20;
  sticker.getContext('2d').fillStyle = '#00ff22'; sticker.getContext('2d').fillRect(0, 0, 20, 20);
  const crop = { x: 43, y: 37, width: 200, height: 140 };
  const flat = { ...DEFAULT_STYLE, background: 'transparent', padding: 0, radius: 0, shadow: 0, borderWidth: 0 };
  const annotations = [
    { id: 'mask', type: 'redact', x: 62.4, y: 50.2, width: 28.2, height: 18.5, color: '#ffffff', lineWidth: 3 },
    { id: 'later-sticker', type: 'sticker', x: 43, y: 37, width: 200, height: 140, color: '#ffffff', lineWidth: 3, dataUrl: 'fixture' },
    { id: 'later-mosaic', type: 'mosaic', x: 50, y: 40, width: 100, height: 80, color: '#ffffff', lineWidth: 3 },
  ];
  const options = { crop, stickers: new Map([['fixture', sticker]]) };
  const preview = renderPreview(source, source, annotations, flat, options, { width: 100, height: 70 });
  const previewCtx = preview.getContext('2d'); let nonBlack = 0, checked = 0;
  for (let y = 6; y < 16; y++) for (let x = 9; x < 24; x++) {
    const p = previewCtx.getImageData(x, y, 1, 1).data; checked++;
    if (p[0] || p[1] || p[2] || p[3] !== 255) nonBlack++;
  }
  const outside = [...previewCtx.getImageData(40, 35, 1, 1).data];
  const exportCases = [1, 2].map(scale => {
    const output = renderImage(source, source, annotations, flat, { ...options, scale });
    const pixel = [...output.getContext('2d').getImageData((70 - crop.x) * scale, (60 - crop.y) * scale, 1, 1).data];
    return { scale, width: output.width, height: output.height, pixel };
  });
  const benchmarkSource = document.createElement('canvas'); benchmarkSource.width = 3840; benchmarkSource.height = 2160;
  const benchmarkCtx = benchmarkSource.getContext('2d'); benchmarkCtx.fillStyle = '#17334f'; benchmarkCtx.fillRect(0, 0, 3840, 2160);
  for (let y = 0; y < 2160; y += 72) { benchmarkCtx.fillStyle = y % 144 ? '#4e917f' : '#cab46b'; benchmarkCtx.fillRect(160, y + 12, 3400, 24); }
  const marks = Array.from({ length: 20 }, (_, i) => ({ id: String(i), type: 'arrow', x: 200 + i * 100, y: 400 + i * 30, width: 200, height: 100, color: '#ff9955', lineWidth: 5 }));
  function measure(previewMode) {
    const create = document.createElement.bind(document), allocations = [];
    document.createElement = function (tag, ...args) { const element = create(tag, ...args); if (tag === 'canvas') allocations.push(element); return element; };
    let output;
    const started = performance.now();
    try {
      output = previewMode ? renderPreview(benchmarkSource, { width: 3840, height: 2160 }, marks, DEFAULT_STYLE, {}, { width: 1280, height: 800 }) : renderImage(benchmarkSource, { width: 3840, height: 2160 }, marks, DEFAULT_STYLE);
      output.getContext('2d').getImageData(0, 0, 1, 1); // Include completion of the actual Canvas raster work.
      return { ms: performance.now() - started, width: output.width, height: output.height, allocatedPixels: allocations.reduce((sum, c) => sum + c.width * c.height, 0), maxCanvasPixels: Math.max(...allocations.map(c => c.width * c.height)) };
    } finally { document.createElement = create; for (const c of allocations) { c.width = 1; c.height = 1; } }
  }
  measure(false); measure(true);
  const full = [], reduced = [];
  for (let i = 0; i < 7; i++) { full.push(measure(false)); reduced.push(measure(true)); }
  const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
  return { privacy: { width: preview.width, height: preview.height, nonBlack, checked, outside, exportCases, previewPng: preview.toDataURL('image/png') }, benchmark: { source: [3840, 2160], viewport: [1280, 800], full, reduced, fullMedianMs: median(full.map(x => x.ms)), previewMedianMs: median(reduced.map(x => x.ms)), fullAllocatedPixels: full[0].allocatedPixels, previewAllocatedPixels: reduced[0].allocatedPixels } };
});
assert.deepEqual([evidence.privacy.width, evidence.privacy.height], [100, 70]);
assert.equal(evidence.privacy.nonBlack, 0);
assert.deepEqual(evidence.privacy.outside, [0, 255, 34, 255]);
assert.deepEqual(evidence.privacy.exportCases.map(x => [x.width, x.height, x.pixel]), [[200, 140, [0, 0, 0, 255]], [400, 280, [0, 0, 0, 255]]]);
assert.ok(evidence.benchmark.reduced.every(x => x.maxCanvasPixels <= 4_000_000));
assert.ok(evidence.benchmark.previewAllocatedPixels < evidence.benchmark.fullAllocatedPixels / 4);
const root = fileURLToPath(new URL('../', import.meta.url));
const artifacts = join(root, 'artifacts', 'preview-performance'); await fs.mkdir(artifacts, { recursive: true });
await fs.writeFile(join(artifacts, 'preview-redaction.png'), Buffer.from(evidence.privacy.previewPng.split(',')[1], 'base64'));
delete evidence.privacy.previewPng;
await fs.writeFile(join(artifacts, 'results.json'), JSON.stringify(evidence, null, 2));
console.log({ result: 'passed', ...evidence });
