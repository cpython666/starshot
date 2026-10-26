import type { Rect, Size } from './geometry.ts';

export type OcrLine = Rect & { text: string };
export type OcrResult = Size & { lines: OcrLine[] };
export const PRIVACY_KINDS = { phone: '手机号/电话', email: '邮箱', identity: '身份证号', bank: '银行卡号', credential: '密码/密钥/助记词', address: '姓名/地址' } as const;
export type PrivacyKind = keyof typeof PRIVACY_KINDS;
export type PrivacyCandidate = Rect & { id: string; kinds: PrivacyKind[]; selected: boolean };

function luhn(value: string) {
  let sum = 0;
  for (let i = value.length - 1, alternate = false; i >= 0; i--, alternate = !alternate) {
    let n = Number(value[i]); if (alternate && (n *= 2) > 9) n -= 9; sum += n;
  }
  return sum % 10 === 0 && !/^(\d)\1+$/.test(value);
}
function kindsFor(text: string): PrivacyKind[] {
  const kinds = new Set<PrivacyKind>();
  const t = text.normalize('NFKC');
  if (/(?:^|\D)(?:\+?86[ -]?)?1[3-9](?:[ -]?\d){9}(?!\d)/.test(t) || /\+\d{1,3}[ -]?(?:\d[ -]?){7,13}\d/.test(t) || /(?:电话|手机|tel|phone)\s*[:：]?\s*[+\d][\d ()-]{6,}/i.test(t)) kinds.add('phone');
  if (/[a-z0-9.!#$%&'*+/=?^_`{|}~-]+\s*@\s*[a-z0-9-]+(?:\s*\.\s*[a-z0-9-]+)+/i.test(t)) kinds.add('email');
  if (/(?:^|\D)[1-9]\d{5}[ -]?(?:18|19|20)\d{2}[ -]?(?:0[1-9]|1[0-2])[ -]?(?:0[1-9]|[12]\d|3[01])[ -]?\d{3}[\dXx](?!\d)/.test(t) || /(?:身份证|passport|护照)\s*[:：]?\s*[A-Z\d][A-Z\d -]{5,}/i.test(t)) kinds.add('identity');
  for (const match of t.matchAll(/(?<!\d)(?:\d[ -]?){12,18}\d(?!\d)/g)) if (luhn(match[0].replace(/\D/g, ''))) kinds.add('bank');
  if (/(?:密码|口令|密钥|助记词|私钥|password|passwd|secret|api[ _-]?key|access[ _-]?token|seed[ _-]?phrase|mnemonic)\s*[:：=]\s*\S+/i.test(t) || /(?:sk-(?:proj-)?[A-Za-z\d_-]{12,}|gh[pousr]_[A-Za-z\d]{16,}|github_pat_[A-Za-z\d_]{16,}|AKIA[A-Z\d]{16}|Bearer\s+[A-Za-z\d._-]{12,}|-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----)/.test(t)) kinds.add('credential');
  if (/(?:收件人|联系人|姓名|住址|收货地址|详细地址|address|recipient)\s*[:：]\s*\S{2,}/i.test(t)) kinds.add('address');
  return [...kinds];
}

/** Cover whole OCR lines: estimated character widths must never leave part of a secret visible. */
export function findSensitiveRegions(result: OcrResult, offset = { x: 0, y: 0 }): PrivacyCandidate[] {
  if (![result.width, result.height].every(n => Number.isFinite(n) && n > 0 && n <= 16384) || !Array.isArray(result.lines) || result.lines.length > 10000) throw Error('文字识别返回了无效尺寸');
  const candidates: PrivacyCandidate[] = [];
  let privateBlock = false, seedLines = 0;
  for (const line of result.lines) {
    if (typeof line.text !== 'string' || line.text.length > 10000 || ![line.x, line.y, line.width, line.height].every(Number.isFinite) || line.width <= 0 || line.height <= 0) continue;
    const kinds = kindsFor(line.text);
    if (/-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----/.test(line.text)) privateBlock = true;
    if (privateBlock || seedLines > 0) { if (!kinds.includes('credential')) kinds.push('credential'); seedLines--; }
    if (/-----END (?:[A-Z]+ )?PRIVATE KEY-----/.test(line.text)) privateBlock = false;
    if (/(?:助记词|seed[ _-]?phrase|mnemonic)\s*[:：]?\s*$/i.test(line.text)) { if (!kinds.includes('credential')) kinds.push('credential'); seedLines = 3; }
    if (!kinds.length) continue;
    const margin = Math.max(3, Math.ceil(line.height * .12));
    const x = Math.max(0, Math.floor(line.x) - margin), y = Math.max(0, Math.floor(line.y) - margin);
    const right = Math.min(result.width, Math.ceil(line.x + line.width) + margin), bottom = Math.min(result.height, Math.ceil(line.y + line.height) + margin);
    if (right <= x || bottom <= y) continue;
    candidates.push({ id: `sensitive-${candidates.length}`, x: x + offset.x, y: y + offset.y, width: right - x, height: bottom - y, kinds, selected: true });
  }
  return candidates;
}
