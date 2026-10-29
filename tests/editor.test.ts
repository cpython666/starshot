import assert from 'node:assert/strict';
import test from 'node:test';
import { calculateLayout, DEFAULT_STYLE, renderImage, renderPreview, sanitizeStyle, validateImageSize, type Annotation } from '../src/editor.ts';

test('crop stays in original coordinates while the output centers it inside padding', () => {
  const layout = calculateLayout({ width: 1200, height: 800 }, { ...DEFAULT_STYLE, padding: 40 }, { x: 100, y: 200, width: 400, height: 300 });
  assert.deepEqual(layout, { width: 480, height: 380, crop: { x: 100, y: 200, width: 400, height: 300 }, imageRect: { x: 40, y: 40, width: 400, height: 300 }, scale: 1 });
  const doubled = calculateLayout({ width: 1200, height: 800 }, { ...DEFAULT_STYLE, padding: 40 }, layout.crop, 2);
  assert.equal(doubled.width, 960);
  assert.equal(doubled.height, 760);
  assert.deepEqual(doubled.imageRect, layout.imageRect);
});

test('fixed ratios add canvas around the source without stretching it', () => {
  const layout = calculateLayout({ width: 640, height: 480 }, { ...DEFAULT_STYLE, padding: 0, aspect: '16:9' });
  assert.equal(layout.height, 480);
  assert.equal(layout.width, 854);
  assert.equal(layout.imageRect.width, 640);
  assert.equal(layout.imageRect.x, 107);
  assert.deepEqual(calculateLayout({ width: 640, height: 480 }, { ...DEFAULT_STYLE, padding: 0, aspect: '1:1' }).imageRect, { x: 0, y: 80, width: 640, height: 480 });
});

test('invalid, empty and oversized render requests fail before allocating canvases', () => {
  const style = { ...DEFAULT_STYLE, padding: 0 };
  assert.throws(() => validateImageSize({ width: 0, height: 20 }), /尺寸超出/);
  assert.throws(() => validateImageSize({ width: 10.2, height: 20 }), /尺寸超出/);
  assert.throws(() => validateImageSize({ width: 8000, height: 6000 }), /尺寸超出/);
  assert.throws(() => calculateLayout({ width: 100, height: 100 }, style, { x: 200, y: 200, width: 10, height: 10 }), /至少/);
  assert.throws(() => calculateLayout({ width: 6000, height: 4000 }, style, undefined, 2), /输出尺寸/);
  assert.throws(() => calculateLayout({ width: 100, height: 100 }, style, undefined, 3), /倍率/);
  const crop = calculateLayout({ width: 100, height: 100 }, style, { x: -1, y: -1, width: 11, height: 12 }).crop;
  assert.deepEqual(crop, { x: 0, y: 0, width: 10, height: 11 });
});

test('persisted appearance is bounded and invalid colors use known defaults', () => {
  const style = sanitizeStyle({ padding: -10, radius: Infinity, borderWidth: 100, backgroundColor: 'url(https://example.com)', gradientTo: '#abc' });
  assert.equal(style.padding, 0);
  assert.equal(style.radius, DEFAULT_STYLE.radius);
  assert.equal(style.borderWidth, 24);
  assert.equal(style.backgroundColor, DEFAULT_STYLE.backgroundColor);
  assert.equal(style.gradientTo, '#abc');
  assert.notEqual(style, DEFAULT_STYLE);
});

test('redaction rendering stays opaque and above later stickers, including cropped and doubled output', () => {
  // Command-level regression only; browser tests verify actual Canvas pixel output.
  const originalDocument = globalThis.document;
  const canvases: { width: number; height: number; events: { operation: string; args: unknown[]; color: unknown; alpha: unknown }[]; getContext: () => unknown }[] = [];
  globalThis.document = { createElement() {
    const events: typeof canvases[number]['events'] = [];
    const state: Record<string, unknown> = { globalAlpha: 1 };
    const ctx = new Proxy(state, { get(target, key) {
      if (key in target) return target[key as string];
      return (...args: unknown[]) => events.push({ operation: String(key), args, color: target.fillStyle, alpha: target.globalAlpha });
    } });
    const canvas = { width: 0, height: 0, events, getContext: () => ctx };
    canvases.push(canvas); return canvas;
  } } as unknown as Document;
  try {
    const source = { width: 20, height: 20 } as CanvasImageSource;
    const sticker = { width: 1, height: 1 } as CanvasImageSource;
    const base = { x: 5.2, y: 6.8, width: 5.1, height: 3.3, lineWidth: 1, color: '#ffffff' };
    const annotations: Annotation[] = [{ ...base, id: 'mask', type: 'redact' }, { ...base, id: 'sticker', type: 'sticker', dataUrl: 'decoded-fixture', rotation: 90 }];
    const result = renderImage(source, { width: 20, height: 20 }, annotations, { ...DEFAULT_STYLE, background: 'transparent', padding: 0, radius: 0, shadow: 0, borderWidth: 0 }, { crop: { x: 3, y: 4, width: 12, height: 12 }, scale: 2, stickers: new Map([['decoded-fixture', sticker]]) });
    assert.equal(result.width, 24); assert.equal(result.height, 24);
    const picture = canvases[0]!;
    const pixels = picture.events.filter(event => ['drawImage', 'fillRect'].includes(event.operation));
    assert.equal(pixels[1]!.args[0], sticker);
    assert.deepEqual(pixels.at(-1), { operation: 'fillRect', args: [5, 6, 6, 5], color: '#000000', alpha: 1 });
    assert.deepEqual(picture.events.filter(event => event.operation === 'translate').at(-1)?.args, [-3, -4]);
    assert.deepEqual(canvases[1]!.events.find(event => event.operation === 'scale')?.args, [2, 2]);
    assert.deepEqual(canvases[1]!.events.filter(event => event.operation === 'fillRect').at(-1), { operation: 'fillRect', args: [4, 4, 12, 10], color: '#000000', alpha: 1 });
    assert.throws(() => renderImage(source, { width: 20, height: 20 }, [annotations[1]!], { ...DEFAULT_STYLE, shadow: 0 }), /尚未解码/);

    const emptyMasks: Annotation[] = [[0, 4], [4, 0], [0, 0]].map(([width, height], i) => ({ ...base, id: `empty-${i}`, type: 'redact', width: width!, height: height! }));
    const unchanged = structuredClone(emptyMasks);
    const plain = { ...DEFAULT_STYLE, background: 'transparent' as const, padding: 0, radius: 0, shadow: 0, borderWidth: 0 };
    const crop = { x: 3, y: 4, width: 12, height: 12 };
    // Fractional coordinates must not let the final floor/ceil repaint turn an empty mask into a black line.
    for (const scale of [1, 2] as const) {
      const start = canvases.length;
      renderImage(source, { width: 20, height: 20 }, emptyMasks, plain, { crop, scale });
      assert.deepEqual(canvases.slice(start).flatMap(c => c.events).filter(e => e.operation === 'fillRect'), []);
    }
    const previewStart = canvases.length;
    renderPreview(source, { width: 20, height: 20 }, emptyMasks, plain, { crop }, { width: 6, height: 6 });
    assert.deepEqual(canvases.slice(previewStart).flatMap(c => c.events).filter(e => e.operation === 'fillRect'), []);
    assert.deepEqual(emptyMasks, unchanged);

    const smallMask: Annotation = { ...base, id: 'small', type: 'redact', width: .01, height: .01 };
    const smallStart = canvases.length;
    renderImage(source, { width: 20, height: 20 }, [...emptyMasks, smallMask], plain, { scale: 2 });
    const fills = canvases.slice(smallStart).flatMap(c => c.events).filter(e => e.operation === 'fillRect');
    assert.equal(fills.length, 2, 'a real small mask remains in both drawing passes');
    assert.ok(fills.every(e => e.color === '#000000' && e.alpha === 1 && Number(e.args[2]) > 0 && Number(e.args[3]) > 0));
    for (const change of [{ width: -1, height: 0 }, { width: 0, height: -1 }, { x: NaN, width: 0 }, { y: Infinity, height: 0 }, { width: NaN }, { height: Infinity }]) {
      assert.throws(() => renderImage(source, { width: 20, height: 20 }, [{ ...smallMask, ...change }], plain), /无效坐标|尺寸必须大于零/);
    }
    assert.throws(() => renderImage(source, { width: 20, height: 20 }, Array.from({ length: 1001 }, () => emptyMasks[0]!), plain), /最多支持 1000/);
  } finally {
    if (originalDocument) globalThis.document = originalDocument;
    else Reflect.deleteProperty(globalThis, 'document');
  }
});


test('display preview bounds every raster allocation while export retains full dimensions and source coordinates', () => {
  const originalDocument = globalThis.document;
  const allocated: { width: number; height: number }[] = [];
  const events: { operation: string; args: unknown[] }[] = [];
  globalThis.document = { createElement() {
    const ctx = new Proxy({}, { get: (_target, key) => (...args: unknown[]) => { events.push({ operation: String(key), args }); } });
    const canvas = { width: 0, height: 0, getContext: () => ctx };
    allocated.push(canvas); return canvas;
  } } as unknown as Document;
  try {
    const source = { width: 6000, height: 4000 } as CanvasImageSource;
    const style = { ...DEFAULT_STYLE, background: 'transparent' as const, padding: 0, radius: 0, shadow: 0, borderWidth: 0 };
    const annotation: Annotation = { id: 'original-coordinate', type: 'rect', x: 1200, y: 800, width: 600, height: 400, color: '#ff0000', lineWidth: 6 };
    const preview = renderPreview(source, { width: 6000, height: 4000 }, [annotation], style, {}, { width: 1200, height: 800 });
    assert.deepEqual([preview.width, preview.height], [1200, 800]);
    assert.ok(allocated.every(c => c.width * c.height <= 960_000));
    assert.ok(events.some(e => e.operation === 'strokeRect' && String(e.args) === '1200,800,600,400'));
    assert.ok(events.some(e => e.operation === 'scale' && String(e.args) === '0.2,0.2'));
    const largePreview = renderPreview(source, { width: 6000, height: 4000 }, [], style, {}, { width: 20_000, height: 20_000 });
    assert.ok(largePreview.width * largePreview.height <= 4_000_000);
    const exported = renderImage(source, { width: 6000, height: 4000 }, [annotation], style);
    assert.deepEqual([exported.width, exported.height], [6000, 4000]);
    assert.deepEqual(annotation, { id: 'original-coordinate', type: 'rect', x: 1200, y: 800, width: 600, height: 400, color: '#ff0000', lineWidth: 6 });
    for (const viewport of [{ width: 0, height: 1 }, { width: NaN, height: 1 }, { width: 1, height: Infinity }]) {
      const count = allocated.length;
      assert.throws(() => renderPreview(source, { width: 6000, height: 4000 }, [], style, {}, viewport), /预览区域/);
      assert.equal(allocated.length, count);
    }
  } finally {
    if (originalDocument) globalThis.document = originalDocument;
    else Reflect.deleteProperty(globalThis, 'document');
  }
});
