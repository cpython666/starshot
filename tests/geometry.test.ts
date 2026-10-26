import assert from 'node:assert/strict';
import test from 'node:test';
import { clampRect, chooseRegion, detectRegions, normalizeRect } from '../src/geometry.ts';

test('backwards drags normalize and clips stop at each image edge', () => {
  assert.deepEqual(normalizeRect({ x: 80, y: 60 }, { x: 10, y: 20 }), { x: 10, y: 20, width: 70, height: 40 });
  assert.deepEqual(clampRect({ x: -20, y: 80, width: 150, height: 100 }, { width: 100, height: 100 }), { x: 0, y: 80, width: 100, height: 20 });
  assert.deepEqual(clampRect({ x: 200, y: 200, width: 30, height: 30 }, { width: 100, height: 100 }), { x: 100, y: 100, width: 0, height: 0 });
  assert.throws(() => clampRect({ x: NaN, y: 0, width: 10, height: 10 }, { width: 100, height: 100 }), /无效坐标/);
});

test('prediction chooses smallest containing visible rectangle, never an offscreen area', () => {
  const size = { width: 100, height: 100 };
  const regions = [
    { x: -20, y: 0, width: 120, height: 100, label: '窗口' },
    { x: 10, y: 10, width: 30, height: 30, label: '控件候选' },
    { x: 200, y: 200, width: 10, height: 10, label: '其他显示器' },
  ];
  assert.equal(chooseRegion({ x: 20, y: 20 }, regions, size)?.label, '控件候选');
  assert.deepEqual(chooseRegion({ x: 80, y: 80 }, regions, size), { x: 0, y: 0, width: 100, height: 100, label: '窗口' });
  assert.equal(chooseRegion({ x: 100, y: 50 }, regions, size), null);
  assert.equal(chooseRegion({ x: NaN, y: 0 }, regions, size), null);
});

test('local prediction finds bounded high-contrast rectangles from original fixture pixels', () => {
  const width = 200, height = 160;
  const data = new Uint8ClampedArray(width * height * 4).fill(255);
  for (let y = 30; y < 120; y++) for (let x = 40; x < 160; x++) {
    const at = (y * width + x) * 4;
    data[at] = 30; data[at + 1] = 40; data[at + 2] = 80;
  }
  assert.deepEqual(detectRegions(data, width, height), [{ x: 40, y: 30, width: 120, height: 90, label: '边缘预测区域', kind: 'edge' }]);
  assert.deepEqual(detectRegions(new Uint8ClampedArray(4), width, height), []);
  assert.deepEqual(detectRegions(new Uint8ClampedArray(width * height * 4).fill(255), width, height), []);
});
