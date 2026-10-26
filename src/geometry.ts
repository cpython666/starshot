export interface Point { x: number; y: number }
export interface Size { width: number; height: number }
export interface Rect extends Point, Size {}
export interface Region extends Rect { label: string; kind?: 'window' | 'edge' }

export function normalizeRect(start: Point, end: Point): Rect {
  return { x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width: Math.abs(end.x - start.x), height: Math.abs(end.y - start.y) };
}

export function clampRect(rect: Rect, size: Size): Rect {
  if (![rect.x, rect.y, rect.width, rect.height, size.width, size.height].every(Number.isFinite)) {
    throw new Error('选区包含无效坐标');
  }
  const normalized = normalizeRect(rect, { x: rect.x + rect.width, y: rect.y + rect.height });
  const maxX = Math.max(0, size.width), maxY = Math.max(0, size.height);
  const x = Math.max(0, Math.min(maxX, normalized.x));
  const y = Math.max(0, Math.min(maxY, normalized.y));
  return { x, y, width: Math.max(0, Math.min(maxX, normalized.x + normalized.width) - x), height: Math.max(0, Math.min(maxY, normalized.y + normalized.height) - y) };
}

export function chooseRegion(point: Point, regions: Region[], size: Size): Region | null {
  if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) return null;
  let chosen: Region | null = null;
  for (const region of regions) {
    if (![region.x, region.y, region.width, region.height].every(Number.isFinite)) continue;
    const rect = clampRect(region, size);
    if (rect.width < 2 || rect.height < 2 || point.x < rect.x || point.y < rect.y || point.x >= rect.x + rect.width || point.y >= rect.y + rect.height) continue;
    if (!chosen || rect.width * rect.height < chosen.width * chosen.height) chosen = { ...region, ...rect };
  }
  return chosen;
}

/** Local edge heuristic, not accessibility or semantic UI-element recognition. */
export function detectRegions(data: Uint8ClampedArray, width: number, height: number): Region[] {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 4 || height < 4 || width * height > 40_000_000 || data.length !== width * height * 4) return [];
  // ponytail: bounded sampled edges find rectangular panels; OS window bounds remain the primary prediction source.
  const step = Math.max(1, Math.ceil(Math.max(width, height) / 480));
  const cols = Math.floor(width / step), rows = Math.floor(height / step);
  const contrast = (x: number, y: number, dx: number, dy: number) => {
    const a = ((y * step) * width + x * step) * 4;
    const b = (((y + dy) * step) * width + (x + dx) * step) * 4;
    return Math.abs(data[a]! - data[b]!) + Math.abs(data[a + 1]! - data[b + 1]!) + Math.abs(data[a + 2]! - data[b + 2]!) > 90;
  };
  const edges: { x: number; y: number; end: number }[] = [];
  const minimum = Math.max(8, Math.ceil(40 / step));
  for (let y = 1; y < rows && edges.length < 512; y++) {
    let start = -1;
    for (let x = 1; x <= cols; x++) {
      const edge = x < cols && contrast(x, y, 0, -1);
      if (edge && start < 0) start = x;
      if (!edge && start >= 0) {
        if (x - start >= minimum) edges.push({ x: start, y, end: x });
        start = -1;
      }
    }
  }
  const candidates: Region[] = [];
  for (let i = 0; i < edges.length && candidates.length < 64; i++) {
    const top = edges[i]!;
    for (let j = i + 1; j < edges.length; j++) {
      const bottom = edges[j]!;
      if (bottom.y - top.y < minimum || Math.abs(top.x - bottom.x) > 2 || Math.abs(top.end - bottom.end) > 2) continue;
      let sideHits = 0, samples = 0;
      const stride = Math.max(1, Math.floor((bottom.y - top.y) / 24));
      for (let y = top.y + 1; y < bottom.y; y += stride) {
        samples += 2;
        if (contrast(top.x, y, -1, 0)) sideHits++;
        if (top.end < cols && contrast(top.end, y, -1, 0)) sideHits++;
      }
      if (samples && sideHits / samples >= 0.65) {
        candidates.push({ x: top.x * step, y: top.y * step, width: (top.end - top.x) * step, height: (bottom.y - top.y) * step, label: '边缘预测区域', kind: 'edge' });
        break;
      }
    }
  }
  return candidates;
}
