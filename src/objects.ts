import type { Annotation } from './editor.ts';
import type { Point, Rect, Size } from './geometry.ts';
import { normalizeRect } from './geometry.ts';

export const MAX_STICKER_BYTES = 2 * 1024 * 1024;
export const MAX_STICKER_PIXELS = 2_000_000;
export const MAX_STICKERS_BYTES = 8 * 1024 * 1024;
export const MAX_STICKERS_PIXELS = 8_000_000;

/** Check the bounded PNG header before allocating or decoding the full image. */
export function stickerInfo(dataUrl: string): Size & { bytes: number } {
  const prefix = 'data:image/png;base64,';
  if (typeof dataUrl !== 'string' || !dataUrl.startsWith(prefix)) throw Error('贴纸仅支持内嵌 PNG 图片');
  if (dataUrl.length > prefix.length + Math.ceil(MAX_STICKER_BYTES / 3) * 4) throw Error('单张贴纸不能超过 2 MiB');
  const body = dataUrl.slice(prefix.length);
  if (!body.length || body.length % 4 !== 0 || !/^[A-Za-z0-9+/]+={0,2}$/.test(body)) throw Error('贴纸图片编码无效');
  const bytes = body.length / 4 * 3 - (body.endsWith('==') ? 2 : body.endsWith('=') ? 1 : 0);
  if (bytes > MAX_STICKER_BYTES) throw Error('单张贴纸不能超过 2 MiB');
  const header = Uint8Array.from(atob(body.slice(0, 44)), char => char.charCodeAt(0));
  const signature = [137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82];
  if (header.length < 33 || signature.some((value, index) => header[index] !== value)) throw Error('贴纸不是有效的 PNG 图片');
  const view = new DataView(header.buffer);
  const width = view.getUint32(16), height = view.getUint32(20);
  if (!width || !height || width > 16384 || height > 16384 || width * height > MAX_STICKER_PIXELS) throw Error('单张贴纸不能超过 200 万像素或单边 16384 像素');
  return { width, height, bytes };
}

/** Count instances, including repeated images, across the caller's whole session. */
export function validateStickerBudget(annotations: readonly Annotation[]): void {
  let bytes = 0, pixels = 0;
  const known = new Map<string, ReturnType<typeof stickerInfo>>();
  for (const annotation of annotations) {
    if (annotation.type !== 'sticker') continue;
    const dataUrl = annotation.dataUrl ?? '';
    const info = known.get(dataUrl) ?? stickerInfo(dataUrl);
    known.set(dataUrl, info);
    bytes += info.bytes; pixels += info.width * info.height;
    if (bytes > MAX_STICKERS_BYTES || pixels > MAX_STICKERS_PIXELS) throw Error('会话贴纸合计不能超过 8 MiB 或 800 万像素');
  }
}

/** Geometry bounds; sticker bounds are before its rotation, for size controls. */
export function annotationBounds(annotation: Annotation): Rect {
  const a = annotation;
  if ((a.type === 'pen' || a.type === 'highlighter') && a.points?.length) {
    let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
    for (const point of a.points) { left = Math.min(left, point.x); top = Math.min(top, point.y); right = Math.max(right, point.x); bottom = Math.max(bottom, point.y); }
    return { x: left, y: top, width: right - left, height: bottom - top };
  }
  if (a.type === 'number') {
    const radius = Math.max(16, Math.min(128, a.fontSize ?? 26)) / 1.5;
    return { x: a.x - radius, y: a.y - radius, width: radius * 2, height: radius * 2 };
  }
  if (a.type === 'text') {
    const fontSize = Math.max(8, Math.min(256, a.fontSize ?? 26));
    const lines = (a.text ?? '').split('\n');
    // ponytail: approximate glyph widths for selection; actual text uses system fonts.
    const width = Math.max(fontSize / 2, ...lines.map(line => Array.from(line).reduce((sum, char) => sum + (/[^\x00-\xff]/.test(char) ? fontSize : fontSize * .65), 0)));
    return { x: a.x, y: a.y, width, height: Math.max(1, lines.length) * fontSize * 1.3 };
  }
  return normalizeRect({ x: a.x, y: a.y }, { x: a.x + a.width, y: a.y + a.height });
}

function inside(point: Point, rect: Rect, tolerance: number): boolean {
  return point.x >= rect.x - tolerance && point.x <= rect.x + rect.width + tolerance && point.y >= rect.y - tolerance && point.y <= rect.y + rect.height + tolerance;
}
function segmentDistance(point: Point, start: Point, end: Point): number {
  const dx = end.x - start.x, dy = end.y - start.y;
  const length = dx * dx + dy * dy;
  const ratio = length ? Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / length)) : 0;
  return Math.hypot(point.x - start.x - dx * ratio, point.y - start.y - dy * ratio);
}

export function hitTestAnnotation(annotation: Annotation, point: Point, tolerance = 6): boolean {
  if (![point.x, point.y, tolerance].every(Number.isFinite) || tolerance < 0) return false;
  const a = annotation, bounds = annotationBounds(a), stroke = Math.max(1, Math.min(80, a.lineWidth));
  if (a.type === 'sticker') {
    const cx = bounds.x + bounds.width / 2, cy = bounds.y + bounds.height / 2;
    const angle = -(a.rotation ?? 0) * Math.PI / 180, dx = point.x - cx, dy = point.y - cy;
    return inside({ x: cx + dx * Math.cos(angle) - dy * Math.sin(angle), y: cy + dx * Math.sin(angle) + dy * Math.cos(angle) }, bounds, tolerance);
  }
  if (a.type === 'pen' || a.type === 'highlighter') {
    const points = a.points?.length ? a.points : [{ x: a.x, y: a.y }, { x: a.x + a.width, y: a.y + a.height }];
    const reach = tolerance + stroke * (a.type === 'highlighter' ? 5 : 1) / 2;
    return points.some((current, index) => segmentDistance(point, points[Math.max(0, index - 1)]!, current) <= reach);
  }
  if (a.type === 'arrow') {
    const end = { x: a.x + a.width, y: a.y + a.height }, angle = Math.atan2(a.height, a.width), head = Math.max(12, stroke * 4);
    return [a, { x: end.x - head * Math.cos(angle - Math.PI / 6), y: end.y - head * Math.sin(angle - Math.PI / 6) }, { x: end.x - head * Math.cos(angle + Math.PI / 6), y: end.y - head * Math.sin(angle + Math.PI / 6) }].some(start => segmentDistance(point, start, end) <= tolerance + stroke / 2);
  }
  if (a.type === 'rect') {
    const reach = tolerance + stroke / 2;
    return inside(point, bounds, reach) && (Math.abs(point.x - bounds.x) <= reach || Math.abs(point.x - bounds.x - bounds.width) <= reach || Math.abs(point.y - bounds.y) <= reach || Math.abs(point.y - bounds.y - bounds.height) <= reach);
  }
  if (a.type === 'ellipse') {
    if (!inside(point, bounds, tolerance + stroke / 2)) return false;
    const rx = bounds.width / 2, ry = bounds.height / 2;
    if (!rx || !ry) return inside(point, bounds, tolerance + stroke / 2);
    const distance = Math.hypot((point.x - bounds.x - rx) / rx, (point.y - bounds.y - ry) / ry);
    return Math.abs(distance - 1) * Math.min(rx, ry) <= tolerance + stroke / 2;
  }
  if (a.type === 'number') return Math.hypot(point.x - a.x, point.y - a.y) <= bounds.width / 2 + tolerance;
  return inside(point, bounds, tolerance);
}

export function findAnnotationAt(annotations: readonly Annotation[], point: Point, tolerance = 6): Annotation | undefined {
  // Redactions render above every other object regardless of insertion order.
  for (const redactions of [true, false]) for (let index = annotations.length - 1; index >= 0; index--) {
    const annotation = annotations[index]!;
    if ((annotation.type === 'redact') === redactions && hitTestAnnotation(annotation, point, tolerance)) return annotation;
  }
  return undefined;
}

/** Return a fresh object; the editor decides its document/crop movement limits. */
export function translateAnnotation(annotation: Annotation, dx: number, dy: number): Annotation {
  if (![dx, dy].every(Number.isFinite)) throw Error('移动距离无效');
  return { ...annotation, x: annotation.x + dx, y: annotation.y + dy, ...(annotation.points ? { points: annotation.points.map(point => ({ x: point.x + dx, y: point.y + dy })) } : {}) };
}

export function resizeAnnotation(annotation: Annotation, target: Rect): Annotation {
  if (![target.x, target.y, target.width, target.height].every(Number.isFinite) || target.width <= 0 || target.height <= 0) throw Error('对象尺寸必须大于零');
  const bounds = annotationBounds(annotation), sx = bounds.width ? target.width / bounds.width : 1, sy = bounds.height ? target.height / bounds.height : 1;
  const map = (point: Point): Point => ({ x: target.x + (bounds.width ? (point.x - bounds.x) * sx : target.width / 2), y: target.y + (bounds.height ? (point.y - bounds.y) * sy : target.height / 2) });
  const start = map(annotation), end = map({ x: annotation.x + annotation.width, y: annotation.y + annotation.height });
  const result = { ...annotation, ...start, width: end.x - start.x, height: end.y - start.y };
  if (annotation.points) result.points = annotation.points.map(map);
  if (annotation.type === 'text' || annotation.type === 'number') result.fontSize = Math.max(annotation.type === 'number' ? 16 : 8, Math.min(annotation.type === 'number' ? 128 : 256, (annotation.fontSize ?? 26) * Math.min(sx, sy)));
  return result;
}
