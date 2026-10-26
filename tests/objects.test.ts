import assert from 'node:assert/strict';
import test from 'node:test';
import type { Annotation } from '../src/editor.ts';
import { annotationBounds, findAnnotationAt, hitTestAnnotation, resizeAnnotation, stickerInfo, translateAnnotation, validateStickerBudget } from '../src/objects.ts';

const pixel = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLttAAAAABJRU5ErkJggg==';
const object = (extra: Partial<Annotation> = {}): Annotation => ({ id: 'a', type: 'rect', x: 10, y: 20, width: 100, height: 60, color: '#123456', lineWidth: 4, ...extra });
function pngHeader(width: number, height: number): string {
  const bytes = Buffer.from(pixel.split(',')[1]!, 'base64');
  bytes.writeUInt32BE(width, 16); bytes.writeUInt32BE(height, 20);
  return `data:image/png;base64,${bytes.toString('base64')}`;
}

test('sticker headers reject foreign formats, malformed data and per-image resource excess', () => {
  assert.deepEqual(stickerInfo(pixel), { width: 1, height: 1, bytes: 70 });
  assert.throws(() => stickerInfo('https://example.com/sticker.png'), /内嵌 PNG/);
  assert.throws(() => stickerInfo(pixel.replace('image/png', 'image/jpeg')), /内嵌 PNG/);
  assert.throws(() => stickerInfo('data:image/png;base64,AAAA'), /有效的 PNG/);
  assert.throws(() => stickerInfo(pixel + '!'), /编码/);
  assert.throws(() => stickerInfo(pngHeader(2001, 1000)), /200 万/);
  assert.throws(() => stickerInfo(pngHeader(20000, 1)), /16384/);
  assert.throws(() => stickerInfo('data:image/png;base64,' + Buffer.alloc(2 * 1024 * 1024 + 1).toString('base64')), /2 MiB/);
});

test('session sticker budget counts repeated instances rather than only distinct image sources', () => {
  const sticker = object({ type: 'sticker', dataUrl: pngHeader(2000, 1000) });
  assert.doesNotThrow(() => validateStickerBudget(Array.from({ length: 4 }, () => sticker)));
  assert.throws(() => validateStickerBudget(Array.from({ length: 5 }, () => sticker)), /800 万/);
  const bytes = Buffer.alloc(2 * 1024 * 1024); Buffer.from(pixel.split(',')[1]!, 'base64').copy(bytes);
  const large = object({ type: 'sticker', dataUrl: `data:image/png;base64,${bytes.toString('base64')}` });
  assert.doesNotThrow(() => validateStickerBudget(Array.from({ length: 4 }, () => large)));
  assert.throws(() => validateStickerBudget(Array.from({ length: 5 }, () => large)), /8 MiB/);
});

test('selection follows outlines, rotated sticker footprints and final redaction stacking', () => {
  assert.equal(hitTestAnnotation(object(), { x: 50, y: 50 }, 0), false);
  assert.equal(hitTestAnnotation(object(), { x: 10, y: 50 }, 0), true);
  const sticker = object({ id: 'sticker', type: 'sticker', width: 100, height: 20, rotation: 90, dataUrl: pixel });
  assert.equal(hitTestAnnotation(sticker, { x: 60, y: 70 }, 0), true);
  assert.equal(hitTestAnnotation(sticker, { x: 15, y: 30 }, 0), false);
  const mask = object({ id: 'mask', type: 'redact' });
  assert.equal(findAnnotationAt([mask, sticker], { x: 60, y: 40 }, 0)?.id, 'mask');
  assert.equal(findAnnotationAt([object({ id: 'bottom', type: 'mosaic' }), object({ id: 'top', type: 'mosaic' })], { x: 60, y: 40 }, 0)?.id, 'top');
  assert.equal(hitTestAnnotation(object({ type: 'ellipse', width: 1000, height: 1 }), { x: 6000, y: 20 }), false);
});

test('stroke movement and resizing transform every point without modifying undo snapshots', () => {
  const pen = object({ type: 'pen', x: 80, y: 10, width: -60, height: 50, points: [{ x: 80, y: 10 }, { x: 20, y: 60 }, { x: 90, y: 30 }] });
  const snapshot = structuredClone(pen);
  assert.deepEqual(annotationBounds(pen), { x: 20, y: 10, width: 70, height: 50 });
  const moved = translateAnnotation(pen, -10, 20);
  assert.deepEqual(moved.points, [{ x: 70, y: 30 }, { x: 10, y: 80 }, { x: 80, y: 50 }]);
  assert.deepEqual(resizeAnnotation(pen, { x: 0, y: 0, width: 140, height: 100 }).points, [{ x: 120, y: 0 }, { x: 0, y: 100 }, { x: 140, y: 40 }]);
  assert.equal(hitTestAnnotation(pen, { x: 50, y: 35 }, 0), true);
  assert.equal(hitTestAnnotation(pen, { x: 20, y: 10 }, 0), false);
  assert.deepEqual(pen, snapshot);
  assert.throws(() => translateAnnotation(pen, NaN, 0), /移动距离/);
  assert.throws(() => resizeAnnotation(pen, { x: 0, y: 0, width: 0, height: 10 }), /大于零/);
});

test('reverse arrows retain direction and number/text bounds include their visible contents', () => {
  const arrow = object({ type: 'arrow', x: 100, y: 100, width: -50, height: -50 });
  const resized = resizeAnnotation(arrow, { x: 10, y: 20, width: 100, height: 200 });
  assert.deepEqual([resized.x, resized.y, resized.width, resized.height], [110, 220, -100, -200]);
  assert.equal(hitTestAnnotation(arrow, { x: 75, y: 75 }, 0), true);
  assert.equal(hitTestAnnotation(arrow, { x: 100, y: 50 }, 0), false);
  const number = object({ type: 'number', fontSize: 30 });
  assert.deepEqual(annotationBounds(number), { x: -10, y: 0, width: 40, height: 40 });
  assert.equal(hitTestAnnotation(number, { x: 25, y: 35 }, 0), false);
  const text = object({ type: 'text', text: '星镜\nabc', fontSize: 20, width: 1, height: 1 });
  assert.deepEqual(annotationBounds(text), { x: 10, y: 20, width: 40, height: 52 });
  assert.equal(hitTestAnnotation(text, { x: 45, y: 65 }, 0), true);
});
