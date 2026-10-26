import { sanitizeStyle, type EditorStyle } from './editor.ts';

export type Appearance = { enabled: boolean; from: string; to: string; angle: number };
export type SavedStyle = { name: string; style: EditorStyle };
export type Preferences = {
  captureDelay: number; format: 'png' | 'jpeg'; outputScale: 1 | 2;
  appearance: Appearance; presets: SavedStyle[];
};
export const PREFERENCES_KEY = 'starshot-preferences';
export function normalizePreferences(value: unknown): Preferences {
  const p = value && typeof value === 'object' ? value as Partial<Preferences> : {};
  const a = p.appearance;
  const hex = (v: unknown, fallback: string) => typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v) ? v : fallback;
  return {
    captureDelay: [0, 3, 5, 10].includes(p.captureDelay!) ? p.captureDelay! : 0,
    format: p.format === 'jpeg' ? 'jpeg' : 'png', outputScale: p.outputScale === 2 ? 2 : 1,
    appearance: { enabled: a?.enabled === true, from: hex(a?.from, '#aa90e5'), to: hex(a?.to, '#617aaa'), angle: Number.isFinite(a?.angle) ? Math.max(0, Math.min(360, a!.angle)) : 135 },
    // Preferences never contain image pixels, including backgrounds and stickers.
    presets: Array.isArray(p.presets) ? p.presets.slice(0, 12).filter(x => x && typeof x.name === 'string' && x.style && x.style.background !== 'image').map(x => ({ name: x.name.trim().slice(0, 30) || '我的美化', style: sanitizeStyle(x.style) })) : [],
  };
}
export function readPreferences(storage: Pick<Storage, 'getItem'>): Preferences {
  try { return normalizePreferences(JSON.parse(storage.getItem(PREFERENCES_KEY) || '{}')); }
  catch { return normalizePreferences(null); }
}
export function applyAppearance(root: HTMLElement, appearance: Appearance, light: boolean) {
  for (const key of ['--accent', '--accent-solid', '--accent-text', '--soft', '--header']) root.style.removeProperty(key);
  if (!appearance.enabled) return;
  const rgb = (hex: string) => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));
  const channels = rgb(appearance.from);
  const luminance = channels.map(v => { const c = v / 255; return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4; }).reduce((n, c, i) => n + c * [0.2126, 0.7152, 0.0722][i]!, 0);
  const tint = channels.map(v => Math.round(v * .5 + (light ? 0 : 255) * .5));
  root.style.setProperty('--accent', `rgb(${tint.join(',')})`);
  root.style.setProperty('--accent-solid', appearance.from);
  root.style.setProperty('--accent-text', luminance > .179 ? '#111111' : '#ffffff');
  root.style.setProperty('--soft', `rgba(${channels.join(',')},.14)`);
  root.style.setProperty('--header', `linear-gradient(${appearance.angle}deg,rgba(${channels.join(',')},.12),rgba(${rgb(appearance.to).join(',')},.12)),var(--panel)`);
}
