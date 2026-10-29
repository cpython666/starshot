export interface Point { x: number; y: number }
export interface Size { width: number; height: number }
export interface Rect extends Point, Size {}
export interface Region extends Rect { label: string; kind?: 'window' | 'edge' | 'element' }

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

/** Keep the strongest initial hit, then only strictly enclosing candidates. */
export function captureRegionsAt(point: Point, windows: Region[], edges: Region[], size: Size, element?: Region | Region[]): Region[] {
  if (![point.x, point.y, size.width, size.height].every(Number.isFinite) || point.x < 0 || point.y < 0 || point.x >= size.width || point.y >= size.height) return [];
  const contains = (r: Rect) => point.x >= r.x && point.y >= r.y && point.x < r.x + r.width && point.y < r.y + r.height;
  const front = windows.find(contains);
  const choices = edges.filter(r => contains(r) && (!front || r.x >= front.x - 2 && r.y >= front.y - 2 && r.x + r.width <= front.x + front.width + 2 && r.y + r.height <= front.y + front.height + 2));
  const append = (region: Region) => {
    // A near-identical pixel edge must not remove the exact window/full-screen endpoint.
    if (!choices.some(r => r.x === region.x && r.y === region.y && r.width === region.width && r.height === region.height)) choices.push(region);
  };
  if (front) append(front);
  append({ x: 0, y: 0, width: size.width, height: size.height, label: '整个画面' });
  // A native control hit is stronger evidence than pixel edges, but never let it
  // escape the foreground window or select a stale point after the cursor moves.
  const semantic = (Array.isArray(element) ? element : element ? [element] : []).slice(0, 8).filter(r => front
    && [r.x, r.y, r.width, r.height].every(Number.isFinite) && r.width >= 2 && r.height >= 2 && contains(r)
    && r.x >= front.x && r.y >= front.y && r.x + r.width <= front.x + front.width && r.y + r.height <= front.y + front.height);
  const candidates: Region[] = [];
  for (const region of [...semantic, ...choices]) {
    if (!candidates.some(r => r.x === region.x && r.y === region.y && r.width === region.width && r.height === region.height)) candidates.push(region);
  }
  // A smaller area alone is not a parent/child relationship. Retain the native
  // semantic hit (e.g. button, not its text), then choose the nearest enclosure.
  // Pixel candidates may supply an intermediate enclosing frame, never a crossing one.
  const result = candidates.length ? [candidates[0]!] : [];
  for (const region of candidates.sort((a, b) => a.width * a.height - b.width * b.height)) {
    const previous = result.at(-1)!;
    if (region.width * region.height > previous.width * previous.height
      && region.x <= previous.x && region.y <= previous.y
      && region.x + region.width >= previous.x + previous.width && region.y + region.height >= previous.y + previous.height) result.push(region);
  }
  return result;
}

/** Local straight-edge heuristic, not accessibility or semantic UI-element recognition. */
export function detectRegions(data: Uint8ClampedArray, width: number, height: number): Region[] {
  // The caller supplies a reduced probe, not the full Retina screenshot. Do not sample it
  // again: skipping every third pixel used to erase thin borders and quantize every box.
  const pixels = width * height;
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 4 || height < 4 || pixels > 4_000_000 || data.length !== pixels * 4) return [];
  const edges = new Uint8Array(pixels);
  const contrast = (a: number, b: number) => Math.abs(data[a]! - data[b]!) + Math.abs(data[a + 1]! - data[b + 1]!) + Math.abs(data[a + 2]! - data[b + 2]!) >= 24;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const index = y * width + x, offset = index * 4;
      // Keep orientation: isolated text strokes are not evidence of all four sides.
      if (y && contrast(offset, offset - width * 4)) edges[index]! |= 1;
      if (x && contrast(offset, offset - 4)) edges[index]! |= 2;
    }
  }

  const queue = new Uint32Array(pixels);
  const candidates: Region[] = [];
  const sideCoverage = (start: number, end: number, fixed: number, horizontal: boolean) => {
    const stride = Math.max(1, Math.ceil((end - start) / 96));
    let hits = 0, samples = 0;
    for (let at = start; at <= end; at += stride) {
      samples++;
      for (let delta = -1; delta <= 1; delta++) {
        const x = horizontal ? at : fixed + delta, y = horizontal ? fixed + delta : at;
        if (x >= 0 && x < width && y >= 0 && y < height && (edges[y * width + x]! & (horizontal ? 1 : 2))) { hits++; break; }
      }
    }
    return hits / Math.max(1, samples);
  };
  // Connected outlines preserve rounded corners, nested panels and short broken borders.
  // The queue and scan are bounded by 4 MP; no all-pairs line search runs on pointermove.
  for (let seed = 0; seed < pixels; seed++) {
    if (!edges[seed] || (edges[seed]! & 4)) continue;
    let read = 0, write = 1;
    queue[0] = seed; edges[seed]! |= 4;
    let left = width, top = height, right = 0, bottom = 0;
    while (read < write) {
      const index = queue[read++]!, x = index % width, y = Math.floor(index / width);
      left = Math.min(left, x); right = Math.max(right, x);
      top = Math.min(top, y); bottom = Math.max(bottom, y);
      for (let dy = -1; dy <= 1; dy++) {
        if (y + dy < 0 || y + dy >= height) continue;
        for (let dx = -1; dx <= 1; dx++) {
          if ((!dx && !dy) || x + dx < 0 || x + dx >= width) continue;
          const adjacent = index + dy * width + dx;
          if (edges[adjacent] && !(edges[adjacent]! & 4)) {
            edges[adjacent]! |= 4; queue[write++] = adjacent;
          }
        }
      }
    }
    const w = right - left, h = bottom - top;
    if (w < 24 || h < 18 || w * h < 640 || w > h * 40 || h > w * 20 || write < (w + h)) continue;
    // A connected photograph/texture is not a rectangular outline.
    if (write > w * h * 0.35 + (w + h) * 2) continue;
    // Ignore corner arcs when checking straight support, but retain their outer bounds.
    const inset = Math.min(18, Math.max(2, Math.floor(Math.min(w, h) * 0.18)));
    const coverage = [
      sideCoverage(left + inset, right - inset, top, true),
      sideCoverage(left + inset, right - inset, bottom, true),
      sideCoverage(top + inset, bottom - inset, left, false),
      sideCoverage(top + inset, bottom - inset, right, false),
    ].sort((a, b) => a - b);
    if (!(coverage[0]! >= 0.5 && coverage[1]! >= 0.75 && coverage[2]! >= 0.8 && coverage[3]! >= 0.8)) continue;
    candidates.push({ x: left, y: top, width: w, height: h, label: '边缘预测区域', kind: 'edge' });
  }
  // Stroke outlines may produce an inner and outer loop. Preserve the outer pixels,
  // without merging genuinely nested cards or controls separated by more than 4 px.
  candidates.sort((a, b) => b.width * b.height - a.width * a.height);
  const unique: Region[] = [];
  for (const candidate of candidates) {
    if (unique.some(other => Math.abs(candidate.x - other.x) <= 4 && Math.abs(candidate.y - other.y) <= 4 && Math.abs(candidate.x + candidate.width - other.x - other.width) <= 4 && Math.abs(candidate.y + candidate.height - other.y - other.height) <= 4)) continue;
    unique.push(candidate);
    if (unique.length === 256) break;
  }
  return unique;
}
