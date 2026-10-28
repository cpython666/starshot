import assert from 'node:assert/strict';
import test from 'node:test';
import { captureRegionsAt, clampRect, chooseRegion, detectRegions, normalizeRect } from '../src/geometry.ts';

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

function fixture(width: number, height: number) {
  const data = new Uint8ClampedArray(width * height * 4).fill(255);
  function fill(x: number, y: number, w: number, h: number, color: number, radius = 0) {
    for (let py = y; py < y + h; py++) for (let px = x; px < x + w; px++) {
      const dx = px + 0.5 - Math.max(x + radius, Math.min(x + w - radius, px + 0.5));
      const dy = py + 0.5 - Math.max(y + radius, Math.min(y + h - radius, py + 0.5));
      if (dx * dx + dy * dy > radius * radius) continue;
      const at = (py * width + px) * 4;
      data[at] = color; data[at + 1] = color; data[at + 2] = color;
    }
  }
  return { data, fill, regions: () => detectRegions(data, width, height) };
}
const bounds = ({ x, y, width, height }: { x: number; y: number; width: number; height: number }) => ({ x, y, width, height });

test('thin low-contrast borders retain exact pixels at the actual 1440 px probe size', () => {
  const image = fixture(1440, 900);
  image.fill(103, 77, 403, 201, 235);
  image.fill(104, 78, 401, 199, 255);
  image.fill(1103, 777, 183, 63, 237);
  image.fill(1104, 778, 181, 61, 255);
  assert.deepEqual(image.regions().map(bounds), [
    { x: 103, y: 77, width: 403, height: 201 },
    { x: 1103, y: 777, width: 183, height: 63 },
  ]);
});

test('rounded outlines, a partly interrupted side and nested controls stay distinct', () => {
  const image = fixture(420, 300);
  image.fill(20, 20, 360, 250, 220, 12);
  image.fill(21, 21, 358, 248, 255, 11);
  image.fill(55, 55, 240, 150, 210, 10);
  image.fill(56, 56, 238, 148, 255, 9);
  // A short break in a border does not make the rest of its rectangle disappear.
  image.fill(135, 55, 45, 1, 255);
  image.fill(80, 90, 72, 30, 230, 5);
  assert.deepEqual(image.regions().map(bounds), [
    { x: 20, y: 20, width: 360, height: 250 },
    { x: 55, y: 55, width: 240, height: 150 },
    { x: 80, y: 90, width: 72, height: 30 },
  ]);
  assert.deepEqual(bounds(chooseRegion({ x: 100, y: 105 }, image.regions(), { width: 420, height: 300 })!), { x: 80, y: 90, width: 72, height: 30 });
});

test('text-like strokes, rules and open corners do not hide a panel near the bottom', () => {
  const image = fixture(600, 900);
  for (let row = 0; row < 44; row++) {
    for (let column = 0; column < 55; column++) {
      image.fill(10 + column * 10, 5 + row * 15, 6, 10, 30);
      image.fill(11 + column * 10, 6 + row * 15, 4, 8, 255);
    }
  }
  image.fill(10, 670, 500, 1, 50);
  image.fill(10, 680, 100, 1, 50);
  image.fill(10, 680, 1, 80, 50);
  image.fill(255, 770, 190, 80, 230);
  image.fill(256, 771, 188, 78, 255);
  assert.deepEqual(image.regions().map(bounds), [{ x: 255, y: 770, width: 190, height: 80 }]);
});

test('prediction rejects oversized probes and bounds dense candidates', () => {
  assert.deepEqual(detectRegions(new Uint8ClampedArray(0), 4000, 4000), []);
  const image = fixture(1100, 800);
  for (let row = 0; row < 20; row++) for (let column = 0; column < 22; column++) {
    image.fill(5 + column * 50, 5 + row * 40, 40, 25, 180);
  }
  const result = image.regions();
  assert.equal(result.length, 256);
  assert.ok(result.every(region => region.width === 40 && region.height === 25));
});


test('dense image texture is not reported as one giant rectangle', () => {
  const width = 320, height = 240;
  const data = new Uint8ClampedArray(width * height * 4).fill(255);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const value = (x + y) % 2 ? 10 : 240, at = (y * width + x) * 4;
    data[at] = value; data[at + 1] = value; data[at + 2] = value;
  }
  assert.deepEqual(detectRegions(data, width, height), []);
});


test('a thicker outline keeps the outer border without swallowing a nested box', () => {
  const image = fixture(260, 180);
  image.fill(20, 20, 200, 140, 180);
  image.fill(24, 24, 192, 132, 255);
  image.fill(40, 40, 130, 80, 180);
  image.fill(41, 41, 128, 78, 255);
  assert.deepEqual(image.regions().map(bounds), [
    { x: 20, y: 20, width: 200, height: 140 },
    { x: 40, y: 40, width: 130, height: 80 },
  ]);
});


test('capture hit testing ignores occluded windows and keeps visible children before their front window', () => {
  const size = { width: 1000, height: 700 };
  const front = { x: 100, y: 80, width: 600, height: 500, label: 'front' };
  const behind = { x: 200, y: 160, width: 80, height: 60, label: 'hidden behind front' };
  const child = { x: 180, y: 130, width: 180, height: 100, label: 'visible pixel card' };
  const crossing = { x: 30, y: 30, width: 800, height: 600, label: 'crosses foreground window' };
  const choices = captureRegionsAt({ x: 220, y: 180 }, [front, behind], [child, crossing], size);
  assert.deepEqual(choices.map(r => r.label), ['visible pixel card', 'front', '整个画面']);
  assert.deepEqual(captureRegionsAt({ x: 220, y: 180 }, [front, behind], [], size).map(r => r.label), ['front', '整个画面']);
  assert.deepEqual(captureRegionsAt({ x: 900, y: 600 }, [front, behind], [], size), [{ x: 0, y: 0, ...size, label: '整个画面' }]);
  assert.deepEqual(captureRegionsAt({ x: 1000, y: 600 }, [front], [], size), []);
});
