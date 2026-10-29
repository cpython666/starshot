import type { Annotation, EditorStyle } from './editor.ts';
import { isEmptyRedaction } from './editor.ts';
import type { Rect } from './geometry.ts';
import { MAX_STICKER_PIXELS, stickerInfo, validateStickerBudget } from './objects.ts';

export const SESSION_SCHEMA_VERSION = 2;
export const MAX_SESSION_BYTES = 256 * 1024 * 1024;
export const SESSION_THEMES = ['starry', 'moon', 'plain', 'moss', 'sakura'] as const;
export type SessionTheme = typeof SESSION_THEMES[number];
export type SessionState = { annotations: Annotation[]; crop?: Rect; style: EditorStyle; backgroundDataUrl?: string };
export type SessionDocument = { id: string; name: string; dataUrl: string; width: number; height: number; state: SessionState };
export type Session = { schemaVersion: 2; theme: SessionTheme; currentId: string; documents: SessionDocument[] };

function invalid(detail: string): never { throw Error(`恢复点内容无效：${detail}`); }
function object(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) invalid(label);
  return value as Record<string, unknown>;
}
function string(value: unknown, label: string, max: number, empty = false): string {
  if (typeof value !== 'string' || (!empty && !value.length) || new TextEncoder().encode(value).length > max) invalid(label);
  return value;
}
function number(value: unknown, label: string, min: number, max: number, integer = false): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max || (integer && !Number.isInteger(value))) invalid(label);
  return value;
}
function choice<T extends string>(value: unknown, values: readonly T[], label: string): T {
  if (typeof value !== 'string' || !values.includes(value as T)) invalid(label);
  return value as T;
}
function color(value: unknown): string {
  if (typeof value !== 'string' || !/^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(value)) invalid('颜色');
  return value;
}
function imageUrl(value: unknown): string {
  if (typeof value !== 'string' || value.length > 90_000_000) invalid('图片数据超过 64 MiB');
  const result = value;
  // Only embedded raster pixels are accepted; never fetch external URLs or decode active SVGs.
  if (!/^data:image\/(?:png|jpeg|webp);base64,/.test(result)) invalid('图片仅支持内嵌 PNG、JPEG 或 WebP');
  const body = result.slice(result.indexOf(',') + 1);
  const decodedBytes = body.length / 4 * 3 - (body.endsWith('==') ? 2 : body.endsWith('=') ? 1 : 0);
  if (decodedBytes > 64 * 1024 * 1024) invalid('图片数据超过 64 MiB');
  if (!body.length || body.length % 4 !== 0 || !/^[A-Za-z0-9+/]+={0,2}$/.test(body)) invalid('图片编码');
  return result;
}
function rect(value: unknown, width: number, height: number): Rect {
  const r = object(value, '裁剪');
  const x = number(r.x, '裁剪横坐标', 0, width), y = number(r.y, '裁剪纵坐标', 0, height);
  const w = number(r.width, '裁剪宽度', 1, width), h = number(r.height, '裁剪高度', 1, height);
  if (x + w > width || y + h > height) invalid('裁剪超出原图');
  return { x, y, width: w, height: h };
}
function editorStyle(value: unknown): EditorStyle {
  const s = object(value, '图片外观');
  return {
    background: choice(s.background, ['gradient', 'solid', 'transparent', 'image'], '背景类型'),
    backgroundColor: color(s.backgroundColor), gradientFrom: color(s.gradientFrom), gradientTo: color(s.gradientTo),
    borderColor: color(s.borderColor), gradientAngle: number(s.gradientAngle, '渐变角度', 0, 360),
    padding: number(s.padding, '留白', 0, 400), radius: number(s.radius, '圆角', 0, 240),
    borderWidth: number(s.borderWidth, '边框', 0, 24), shadow: number(s.shadow, '阴影', 0, 80),
    aspect: choice(s.aspect, ['auto', '1:1', '4:3', '16:9', '9:16'], '画布比例'),
  };
}
function annotation(value: unknown, schemaVersion: number): Annotation {
  const a = object(value, '标注');
  const result: Annotation = {
    id: string(a.id, '标注编号', 128),
    type: choice(a.type, ['rect', 'ellipse', 'arrow', 'pen', 'highlighter', 'text', 'number', 'mosaic', 'sticker', 'redact'], '标注类型'),
    x: number(a.x, '标注横坐标', -16384, 16384), y: number(a.y, '标注纵坐标', -16384, 16384),
    width: number(a.width, '标注宽度', -16384, 16384), height: number(a.height, '标注高度', -16384, 16384),
    color: color(a.color), lineWidth: number(a.lineWidth, '标注线宽', 1, 100),
  };
  if (schemaVersion === 1 && (result.type === 'sticker' || result.type === 'redact')) invalid('旧会话版本不能包含贴纸或实心遮挡');
  if (a.points !== undefined) {
    if (!Array.isArray(a.points) || a.points.length > 100_000) invalid('标注点数');
    result.points = a.points.map(p => { const point = object(p, '标注点'); return { x: number(point.x, '标注点横坐标', -16384, 16384), y: number(point.y, '标注点纵坐标', -16384, 16384) }; });
  }
  if (a.text !== undefined) result.text = string(a.text, '标注文字', 10_000, true);
  if (a.fontSize !== undefined) result.fontSize = number(a.fontSize, '文字大小', 1, 256);
  if (a.number !== undefined) result.number = number(a.number, '序号', 1, 100_000, true);
  if (result.type === 'sticker' || result.type === 'redact') {
    if (result.width <= 0 || result.height <= 0) invalid('贴纸和遮挡区域尺寸必须大于零');
  }
  if (result.type === 'sticker') {
    if (typeof a.dataUrl !== 'string') invalid('缺少贴纸图片');
    stickerInfo(a.dataUrl);
    result.dataUrl = a.dataUrl;
    if (a.rotation !== undefined) result.rotation = number(a.rotation, '贴纸旋转角度', 0, 360);
  }
  return result;
}

/** Parse into fresh, known fields, discarding any incidental editor/undo history. */
export function parseSession(serialized: string): Session {
  if (typeof serialized !== 'string' || serialized.length > MAX_SESSION_BYTES || new TextEncoder().encode(serialized).length > MAX_SESSION_BYTES) invalid('会话超过 256 MiB');
  let json: unknown;
  try { json = JSON.parse(serialized); } catch { invalid('不是有效的 JSON'); }
  const data = object(json, '会话');
  if (data.schemaVersion !== 1 && data.schemaVersion !== SESSION_SCHEMA_VERSION) invalid('不支持的会话版本，请升级星镜');
  const theme = choice(data.theme, SESSION_THEMES, '界面外观');
  if (!Array.isArray(data.documents) || data.documents.length > 6) invalid('最多保留 6 张图片');
  let pixels = 0;
  const ids = new Set<string>();
  const documents = data.documents.map(value => {
    const d = object(value, '图片'), s = object(d.state, '编辑状态');
    const id = string(d.id, '图片编号', 128);
    if (ids.has(id)) invalid('图片编号重复');
    ids.add(id);
    const width = number(d.width, '原图宽度', 1, 16384, true), height = number(d.height, '原图高度', 1, 16384, true);
    pixels += width * height;
    if (width * height > 40_000_000 || pixels > 48_000_000) invalid('原图像素超出会话预算');
    if (!Array.isArray(s.annotations) || s.annotations.length > 2000) invalid('标注数量');
    const annotations = s.annotations.map(value => annotation(value, data.schemaVersion as number));
    const annotationIds = new Set(annotations.map(a => a.id));
    if (annotationIds.size !== annotations.length) invalid('标注编号重复');
    const points = annotations.reduce((count, a) => count + (a.points?.length || 0), 0);
    if (points > 100_000) invalid('会话标注点数过多');
    const state: SessionState = { annotations, style: editorStyle(s.style) };
    if (s.crop !== undefined) state.crop = rect(s.crop, width, height);
    if (s.backgroundDataUrl !== undefined) state.backgroundDataUrl = imageUrl(s.backgroundDataUrl);
    if (state.style.background === 'image' && !state.backgroundDataUrl) invalid('缺少背景图片');
    return { id, name: string(d.name, '图片名称', 500), dataUrl: imageUrl(d.dataUrl), width, height, state };
  });
  const currentId = string(data.currentId, '当前图片', 128, true);
  if (documents.length ? !ids.has(currentId) : currentId !== '') invalid('当前图片不存在');
  validateStickerBudget(documents.flatMap(doc => doc.state.annotations));
  return { schemaVersion: SESSION_SCHEMA_VERSION, theme, currentId, documents };
}

export function serializeSession(session: Session): string {
  // Only repair invisible legacy masks in the editor's own state, without mutating undo history.
  const documents = session.documents.map(doc => {
    const annotations = doc.state.annotations;
    if (annotations.length > 2000) invalid('标注数量');
    if (annotations.reduce((count, a) => count + (a.points?.length || 0), 0) > 100_000) invalid('会话标注点数过多');
    return { ...doc, state: { ...doc.state, annotations: annotations.filter(a => !isEmptyRedaction(a)) } };
  });
  // Round-trip validation also strips undo/redo and other runtime fields before persistence.
  return JSON.stringify(parseSession(JSON.stringify({ ...session, documents })));
}

/** Decode every image before the caller atomically replaces the current in-memory session. */
export async function prepareSession<T extends { width: number; height: number }>(serialized: string, decode: (dataUrl: string, maxPixels: number) => Promise<T>) {
  const session = parseSession(serialized);
  const images = new Map<string, T>(), backgrounds = new Map<string, T>(), stickers = new Map<string, T>();
  for (const doc of session.documents) {
    const image = await decode(doc.dataUrl, 40_000_000);
    if (image.width !== doc.width || image.height !== doc.height) invalid('记录尺寸与原图不一致');
    images.set(doc.id, image);
    if (doc.state.backgroundDataUrl) {
      const background = await decode(doc.state.backgroundDataUrl, 24_000_000);
      if (!Number.isInteger(background.width) || !Number.isInteger(background.height) || background.width <= 0 || background.height <= 0 || Math.max(background.width, background.height) > 16384 || background.width * background.height > 24_000_000) invalid('背景尺寸超限');
      backgrounds.set(doc.id, background);
    }
    for (const annotation of doc.state.annotations) {
      if (annotation.type !== 'sticker' || !annotation.dataUrl || stickers.has(annotation.dataUrl)) continue;
      const info = stickerInfo(annotation.dataUrl);
      const image = await decode(annotation.dataUrl, MAX_STICKER_PIXELS);
      if (image.width !== info.width || image.height !== info.height) invalid('贴纸尺寸与图片不一致');
      stickers.set(annotation.dataUrl, image);
    }
  }
  return { session, images, backgrounds, stickers };
}

/** Snapshot first: a failed backup must leave the current session untouched. */
export async function restoreWithBackup<T>(actions: {
  getSession: () => Promise<string>;
  backup: (session: string) => Promise<T>;
  read: () => Promise<string>;
  apply: (session: string) => Promise<void>;
}): Promise<T> {
  const current = await actions.getSession();
  const backup = await actions.backup(current);
  const previous = await actions.read();
  await actions.apply(previous);
  return backup;
}
