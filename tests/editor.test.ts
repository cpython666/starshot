import assert from 'node:assert/strict';
import test from 'node:test';
import { calculateLayout, DEFAULT_STYLE, sanitizeStyle, validateImageSize } from '../src/editor.ts';

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
