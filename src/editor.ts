import { clampRect, normalizeRect } from './geometry.ts';
import type { Point, Rect, Size } from './geometry.ts';

export type AnnotationType = 'rect' | 'ellipse' | 'arrow' | 'pen' | 'highlighter' | 'text' | 'number' | 'mosaic' | 'sticker' | 'redact';
export interface Annotation extends Rect {
  id: string;
  type: AnnotationType;
  color: string;
  lineWidth: number;
  points?: Point[];
  text?: string;
  fontSize?: number;
  number?: number;
  dataUrl?: string;
  rotation?: number;
}

/** A legacy empty mask covers no pixels; malformed or nonempty masks must still be checked. */
export function isEmptyRedaction(annotation: Annotation): boolean {
  return annotation.type === 'redact'
    && [annotation.x, annotation.y, annotation.width, annotation.height].every(Number.isFinite)
    && annotation.width >= 0 && annotation.height >= 0
    && (annotation.width === 0 || annotation.height === 0);
}

export interface EditorStyle {
  background: 'gradient' | 'solid' | 'transparent' | 'image';
  backgroundColor: string;
  gradientFrom: string;
  gradientTo: string;
  gradientAngle: number;
  padding: number;
  radius: number;
  borderWidth: number;
  borderColor: string;
  shadow: number;
  aspect: 'auto' | '1:1' | '4:3' | '16:9' | '9:16';
}
export const DEFAULT_STYLE: EditorStyle = {
  background: 'gradient', backgroundColor: '#171c32', gradientFrom: '#b5a0e2', gradientTo: '#617aaa',
  gradientAngle: 135, padding: 72, radius: 16, borderWidth: 1, borderColor: '#ffffff', shadow: 32, aspect: 'auto',
};
export const MAX_IMAGE_PIXELS = 40_000_000;
export const MAX_RENDER_PIXELS = 64_000_000;
export const MAX_DIMENSION = 16_384;
export interface RenderOptions { crop?: Rect; scale?: 1 | 2; backgroundImage?: CanvasImageSource; stickers?: ReadonlyMap<string, CanvasImageSource>; format?: 'png' | 'jpeg' }
export interface RenderLayout extends Size { imageRect: Rect; crop: Rect; scale: number }

const bounded = (value: number, min: number, max: number, fallback: number) => Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
const color = (value: string, fallback: string) => /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(value) ? value : fallback;

export function sanitizeStyle(style: Partial<EditorStyle>): EditorStyle {
  const current = { ...DEFAULT_STYLE, ...style };
  return {
    background: ['gradient', 'solid', 'transparent', 'image'].includes(current.background) ? current.background : DEFAULT_STYLE.background,
    backgroundColor: color(current.backgroundColor, DEFAULT_STYLE.backgroundColor),
    gradientFrom: color(current.gradientFrom, DEFAULT_STYLE.gradientFrom), gradientTo: color(current.gradientTo, DEFAULT_STYLE.gradientTo),
    gradientAngle: bounded(current.gradientAngle, 0, 360, 135), padding: bounded(current.padding, 0, 400, 72),
    radius: bounded(current.radius, 0, 240, 16), borderWidth: bounded(current.borderWidth, 0, 24, 1),
    borderColor: color(current.borderColor, DEFAULT_STYLE.borderColor), shadow: bounded(current.shadow, 0, 80, 32),
    aspect: ['auto', '1:1', '4:3', '16:9', '9:16'].includes(current.aspect) ? current.aspect : 'auto',
  };
}

export function validateImageSize(size: Size): void {
  if (![size.width, size.height].every(value => Number.isInteger(value) && value > 0 && value <= MAX_DIMENSION) || size.width * size.height > MAX_IMAGE_PIXELS) {
    throw new Error('图片尺寸超出限制：最长边 16384 像素、总计 4000 万像素');
  }
}

export function calculateLayout(size: Size, rawStyle: EditorStyle, selectedCrop?: Rect, scale = 1): RenderLayout {
  validateImageSize(size);
  if (scale !== 1 && scale !== 2) throw new Error('导出倍率仅支持 1× 或 2×');
  const style = sanitizeStyle(rawStyle);
  const clipped = clampRect(selectedCrop ?? { x: 0, y: 0, ...size }, size);
  const x = Math.floor(clipped.x), y = Math.floor(clipped.y);
  const crop = { x, y, width: Math.min(size.width - x, Math.ceil(clipped.x + clipped.width) - x), height: Math.min(size.height - y, Math.ceil(clipped.y + clipped.height) - y) };
  if (clipped.width < 1 || clipped.height < 1) throw new Error('请保留至少 1 × 1 像素的选区');
  let width = crop.width + Math.ceil(style.padding) * 2;
  let height = crop.height + Math.ceil(style.padding) * 2;
  if (style.aspect !== 'auto') {
    const [rw, rh] = style.aspect.split(':').map(Number) as [number, number];
    if (width / height < rw / rh) width = Math.ceil(height * rw / rh);
    else height = Math.ceil(width * rh / rw);
  }
  if (width * scale > MAX_DIMENSION || height * scale > MAX_DIMENSION || width * height * scale * scale > MAX_RENDER_PIXELS) {
    throw new Error('输出尺寸过大，请减少留白、裁剪图片或使用 1× 导出');
  }
  return { width: width * scale, height: height * scale, imageRect: { x: (width - crop.width) / 2, y: (height - crop.height) / 2, width: crop.width, height: crop.height }, crop, scale };
}

function canvas(width: number, height: number): HTMLCanvasElement {
  const element = document.createElement('canvas');
  element.width = width; element.height = height;
  return element;
}
function context(element: HTMLCanvasElement): CanvasRenderingContext2D {
  const ctx = element.getContext('2d');
  if (!ctx) throw new Error('当前环境无法创建图片画布');
  return ctx;
}
function roundedPath(ctx: CanvasRenderingContext2D, rect: Rect, radius: number): void {
  const { x, y, width, height } = rect;
  const r = Math.max(0, Math.min(radius, width / 2, height / 2));
  ctx.beginPath(); ctx.moveTo(x + r, y);
  ctx.lineTo(x + width - r, y); ctx.quadraticCurveTo(x + width, y, x + width, y + r);
  ctx.lineTo(x + width, y + height - r); ctx.quadraticCurveTo(x + width, y + height, x + width - r, y + height);
  ctx.lineTo(x + r, y + height); ctx.quadraticCurveTo(x, y + height, x, y + height - r);
  ctx.lineTo(x, y + r); ctx.quadraticCurveTo(x, y, x + r, y); ctx.closePath();
}

function drawAnnotation(ctx: CanvasRenderingContext2D, annotation: Annotation, stickers?: ReadonlyMap<string, CanvasImageSource>): void {
  if (![annotation.x, annotation.y, annotation.width, annotation.height].every(Number.isFinite)) throw new Error('标注包含无效坐标');
  ctx.save();
  ctx.strokeStyle = ctx.fillStyle = color(annotation.color, '#ff6b81');
  ctx.lineWidth = bounded(annotation.lineWidth, 1, 80, 4);
  ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  const { x, y, width, height } = annotation;
  const rect = normalizeRect({ x, y }, { x: x + width, y: y + height });
  ctx.beginPath();
  switch (annotation.type) {
    case 'sticker': {
      const image = stickers?.get(annotation.dataUrl ?? '');
      if (!image) throw Error('贴纸尚未解码，请重新导入或恢复会话');
      if (width <= 0 || height <= 0) throw Error('贴纸尺寸必须大于零');
      const rotation = annotation.rotation ?? 0;
      if (!Number.isFinite(rotation) || rotation < 0 || rotation > 360) throw Error('贴纸旋转角度无效');
      ctx.translate(x + width / 2, y + height / 2);
      ctx.rotate(rotation * Math.PI / 180);
      ctx.drawImage(image, -width / 2, -height / 2, width, height);
      break;
    }
    case 'redact': {
      if (width <= 0 || height <= 0) throw Error('遮挡区域尺寸必须大于零');
      // Expand fractional edges to whole source pixels, with no source transparency.
      ctx.globalAlpha = 1; ctx.globalCompositeOperation = 'source-over'; ctx.fillStyle = '#000000';
      const left = Math.floor(x), top = Math.floor(y);
      ctx.fillRect(left, top, Math.ceil(x + width) - left, Math.ceil(y + height) - top);
      break;
    }
    case 'rect': ctx.strokeRect(rect.x, rect.y, rect.width, rect.height); break;
    case 'ellipse':
      ctx.ellipse(rect.x + rect.width / 2, rect.y + rect.height / 2, rect.width / 2, rect.height / 2, 0, 0, Math.PI * 2); ctx.stroke(); break;
    case 'arrow': {
      const angle = Math.atan2(height, width), head = Math.max(12, ctx.lineWidth * 4);
      ctx.moveTo(x, y); ctx.lineTo(x + width, y + height); ctx.stroke();
      ctx.beginPath(); ctx.moveTo(x + width, y + height);
      ctx.lineTo(x + width - head * Math.cos(angle - Math.PI / 6), y + height - head * Math.sin(angle - Math.PI / 6));
      ctx.lineTo(x + width - head * Math.cos(angle + Math.PI / 6), y + height - head * Math.sin(angle + Math.PI / 6));
      ctx.closePath(); ctx.fill(); break;
    }
    case 'pen':
    case 'highlighter': {
      const points = annotation.points ?? [{ x, y }, { x: x + width, y: y + height }];
      if (points.length > 20_000) throw new Error('单笔标注过长，请分成多笔');
      if (annotation.type === 'highlighter') { ctx.globalAlpha = 0.35; ctx.lineWidth *= 5; }
      for (let i = 0; i < points.length; i++) {
        const p = points[i]!;
        if (!Number.isFinite(p.x) || !Number.isFinite(p.y)) throw new Error('笔迹包含无效坐标');
        if (i === 0) ctx.moveTo(p.x, p.y); else ctx.lineTo(p.x, p.y);
      }
      if (points.length === 1) { ctx.arc(points[0]!.x, points[0]!.y, ctx.lineWidth / 2, 0, Math.PI * 2); ctx.fill(); }
      else ctx.stroke();
      break;
    }
    case 'text': {
      const fontSize = bounded(annotation.fontSize ?? 26, 8, 256, 26);
      ctx.font = `600 ${fontSize}px -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", sans-serif`;
      ctx.textBaseline = 'top';
      const text = annotation.text ?? '';
      if (text.length > 10_000) throw new Error('单条文字标注不能超过 10000 字符');
      text.split('\n').forEach((line, index) => ctx.fillText(line, x, y + index * fontSize * 1.3));
      break;
    }
    case 'number': {
      const radius = bounded(annotation.fontSize ?? 26, 16, 128, 26) / 1.5;
      ctx.arc(x, y, radius, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = '#ffffff'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.font = `700 ${radius * 1.15}px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`;
      ctx.fillText(String(Math.round(bounded(annotation.number ?? 1, 1, 999, 1))), x, y + 1);
      break;
    }
  }
  ctx.restore();
}

function pixelate(target: HTMLCanvasElement, annotation: Annotation, crop: Rect, scale = 1): void {
  const selection = clampRect({ x: (annotation.x - crop.x) * scale, y: (annotation.y - crop.y) * scale, width: annotation.width * scale, height: annotation.height * scale }, target);
  const x = Math.floor(selection.x), y = Math.floor(selection.y);
  const width = Math.min(target.width - x, Math.ceil(selection.x + selection.width) - x);
  const height = Math.min(target.height - y, Math.ceil(selection.y + selection.height) - y);
  if (width < 1 || height < 1) return;
  const block = Math.max(1, Math.max(8, Math.round(bounded(annotation.lineWidth, 1, 80, 4) * 3)) * scale);
  const small = canvas(Math.max(1, Math.ceil(width / block)), Math.max(1, Math.ceil(height / block)));
  const smallCtx = context(small);
  smallCtx.drawImage(target, x, y, width, height, 0, 0, small.width, small.height);
  const targetCtx = context(target);
  targetCtx.save();
  targetCtx.imageSmoothingEnabled = false;
  // Replace the source pixels, including alpha; output contains no hidden original layer.
  targetCtx.clearRect(x, y, width, height);
  targetCtx.drawImage(small, 0, 0, small.width, small.height, x, y, width, height);
  targetCtx.restore();
}

function sourceSize(source: CanvasImageSource): Size {
  if ('naturalWidth' in source) return { width: source.naturalWidth, height: source.naturalHeight };
  if ('videoWidth' in source) return { width: source.videoWidth, height: source.videoHeight };
  if ('displayWidth' in source) return { width: source.displayWidth, height: source.displayHeight };
  return { width: Number(source.width), height: Number(source.height) };
}

export function renderImage(source: CanvasImageSource, size: Size, annotations: Annotation[], rawStyle: EditorStyle, options: RenderOptions = {}): HTMLCanvasElement {
  return render(source, size, annotations, rawStyle, options, 1);
}

/** Display-only raster budget. All editing coordinates and exports stay in source pixels. */
export function renderPreview(source: CanvasImageSource, size: Size, annotations: Annotation[], rawStyle: EditorStyle, options: Omit<RenderOptions, 'scale'>, viewport: Size): HTMLCanvasElement {
  if (![viewport.width, viewport.height].every(value => Number.isFinite(value) && value > 0)) throw new Error('预览区域尺寸无效');
  const layout = calculateLayout(size, rawStyle, options.crop);
  // ponytail: a 4 MP preview bounds drag-time allocation; export still uses the full source.
  const scale = Math.min(1, viewport.width / layout.width, viewport.height / layout.height, Math.sqrt(4_000_000 / (layout.width * layout.height)));
  return render(source, size, annotations, rawStyle, { ...options, scale: 1 }, scale);
}

function render(source: CanvasImageSource, size: Size, annotations: Annotation[], rawStyle: EditorStyle, options: RenderOptions, rasterScale: number): HTMLCanvasElement {
  const style = sanitizeStyle(rawStyle);
  const layout = calculateLayout(size, style, options.crop, options.scale ?? 1);
  if (annotations.length > 1_000) throw new Error('单张图片最多支持 1000 个标注');
  annotations = annotations.filter(annotation => !isEmptyRedaction(annotation));
  const crop = layout.crop;
  const picture = canvas(Math.max(1, Math.floor(crop.width * rasterScale)), Math.max(1, Math.floor(crop.height * rasterScale)));
  const pictureCtx = context(picture);
  pictureCtx.drawImage(source, crop.x, crop.y, crop.width, crop.height, 0, 0, picture.width, picture.height);
  const pictureScaleX = picture.width / crop.width, pictureScaleY = picture.height / crop.height;
  for (const annotation of annotations) {
    if (annotation.type === 'redact') continue;
    if (annotation.type === 'mosaic') { pixelate(picture, annotation, crop, rasterScale); continue; }
    pictureCtx.save(); pictureCtx.scale(pictureScaleX, pictureScaleY); pictureCtx.translate(-crop.x, -crop.y);
    drawAnnotation(pictureCtx, annotation, options.stickers);
    pictureCtx.restore();
  }
  // Keep solid privacy masks above later stickers, highlights and mosaics.
  for (const annotation of annotations) {
    if (annotation.type !== 'redact') continue;
    pictureCtx.save(); pictureCtx.scale(pictureScaleX, pictureScaleY); pictureCtx.translate(-crop.x, -crop.y);
    drawAnnotation(pictureCtx, annotation);
    pictureCtx.restore();
  }
  const output = canvas(Math.max(1, Math.floor(layout.width * rasterScale)), Math.max(1, Math.floor(layout.height * rasterScale)));
  const ctx = context(output);
  const outputScaleX = output.width / (layout.width / layout.scale), outputScaleY = output.height / (layout.height / layout.scale);
  ctx.scale(outputScaleX, outputScaleY);
  const width = layout.width / layout.scale, height = layout.height / layout.scale;
  if (options.format === 'jpeg') { ctx.fillStyle = '#ffffff'; ctx.fillRect(0, 0, width, height); }
  if (style.background === 'solid') { ctx.fillStyle = style.backgroundColor; ctx.fillRect(0, 0, width, height); }
  if (style.background === 'gradient') {
    const angle = (style.gradientAngle - 90) * Math.PI / 180;
    const half = Math.abs(width * Math.cos(angle)) / 2 + Math.abs(height * Math.sin(angle)) / 2;
    const dx = Math.cos(angle) * half, dy = Math.sin(angle) * half;
    const gradient = ctx.createLinearGradient(width / 2 - dx, height / 2 - dy, width / 2 + dx, height / 2 + dy);
    gradient.addColorStop(0, style.gradientFrom); gradient.addColorStop(1, style.gradientTo);
    ctx.fillStyle = gradient; ctx.fillRect(0, 0, width, height);
  }
  if (style.background === 'image') {
    if (!options.backgroundImage) throw new Error('请先选择背景图片');
    const dimensions = sourceSize(options.backgroundImage);
    validateImageSize(dimensions);
    const factor = Math.max(width / dimensions.width, height / dimensions.height);
    ctx.drawImage(options.backgroundImage, (width - dimensions.width * factor) / 2, (height - dimensions.height * factor) / 2, dimensions.width * factor, dimensions.height * factor);
  }
  const imageRect = layout.imageRect;
  if (style.shadow > 0) {
    const shadow = canvas(output.width, output.height), shadowCtx = context(shadow);
    shadowCtx.scale(outputScaleX, outputScaleY);
    shadowCtx.shadowColor = '#090c3266'; shadowCtx.shadowBlur = style.shadow * outputScaleX;
    shadowCtx.shadowOffsetY = style.shadow * outputScaleY / 3;
    shadowCtx.fillStyle = '#151829'; roundedPath(shadowCtx, imageRect, style.radius); shadowCtx.fill();
    shadowCtx.shadowColor = 'transparent'; shadowCtx.globalCompositeOperation = 'destination-out';
    roundedPath(shadowCtx, imageRect, style.radius); shadowCtx.fill();
    ctx.drawImage(shadow, 0, 0, width, height);
  }
  ctx.save(); roundedPath(ctx, imageRect, style.radius); ctx.clip();
  ctx.drawImage(picture, imageRect.x, imageRect.y, imageRect.width, imageRect.height);
  // Resampling at 2× can blend a masked edge with adjacent colored pixels.
  // Repaint in output pixel space, while retaining the image's rounded clip.
  ctx.scale(1 / outputScaleX, 1 / outputScaleY);
  for (const annotation of annotations) {
    if (annotation.type !== 'redact') continue;
    const left = Math.floor((imageRect.x + Math.floor(annotation.x) - crop.x) * outputScaleX);
    const top = Math.floor((imageRect.y + Math.floor(annotation.y) - crop.y) * outputScaleY);
    const right = Math.ceil((imageRect.x + Math.ceil(annotation.x + annotation.width) - crop.x) * outputScaleX);
    const bottom = Math.ceil((imageRect.y + Math.ceil(annotation.y + annotation.height) - crop.y) * outputScaleY);
    ctx.globalAlpha = 1; ctx.globalCompositeOperation = 'source-over'; ctx.fillStyle = '#000000';
    ctx.fillRect(left, top, right - left, bottom - top);
  }
  ctx.restore();
  if (style.borderWidth > 0) {
    const inset = style.borderWidth / 2;
    const borderRect = { x: imageRect.x + inset, y: imageRect.y + inset, width: Math.max(0, imageRect.width - style.borderWidth), height: Math.max(0, imageRect.height - style.borderWidth) };
    if (borderRect.width > 0 && borderRect.height > 0) {
      ctx.strokeStyle = style.borderColor; ctx.lineWidth = style.borderWidth;
      roundedPath(ctx, borderRect, Math.max(0, style.radius - inset)); ctx.stroke();
    }
  }
  return output;
}
