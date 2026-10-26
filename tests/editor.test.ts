import assert from 'node:assert/strict';
import test from 'node:test';
import { calculateLayout, DEFAULT_STYLE, renderImage, sanitizeStyle, validateImageSize, type Annotation } from '../src/editor.ts';

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
  } finally {
    if (originalDocument) globalThis.document = originalDocument;
    else Reflect.deleteProperty(globalThis, 'document');
  }
});
