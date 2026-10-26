import assert from 'node:assert/strict';
import test from 'node:test';
import { findSensitiveRegions } from '../src/privacy.ts';
import { normalizePreferences, readPreferences } from '../src/preferences.ts';

test('offline sensitive rules cover padded whole lines and never expose text in candidates', () => {
  const texts = ['手机：138 0000 0000', 'demo@example.invalid', '身份证：110101199001010012', '银行卡 4111 1111 1111 1111', 'password: fixture-only-secret', '姓名：原创测试', '普通说明 2026年10月26日', '1234567890123456'];
  const result = findSensitiveRegions({ width: 600, height: 400, lines: texts.map((text, i) => ({ text, x: 10, y: i * 40, width: 300, height: 24 })) }, { x: 20, y: 30 });
  assert.equal(result.length, 6);
  assert.deepEqual(result.map(x => x.kinds[0]), ['phone', 'email', 'identity', 'bank', 'credential', 'address']);
  assert.equal(result[0]!.x, 27); assert.equal(result[0]!.y, 30);
  assert.ok(result.every(x => x.width > 300 && !('text' in x)));
});
test('private key lines are covered through END, with invalid boxes rejected', () => {
  const texts = ['-----BEGIN PRIVATE KEY-----', 'fixture-encoded-data', '-----END PRIVATE KEY-----', 'ordinary'];
  const result = findSensitiveRegions({ width: 300, height: 200, lines: texts.map((text, i) => ({ text, x: 0, y: i * 20, width: 250, height: 18 })) });
  assert.equal(result.length, 3);
  assert.throws(() => findSensitiveRegions({width: Infinity, height: 1, lines: []}));
});
test('preferences tolerate corruption and never preserve image backgrounds or incidental fields', () => {
  const p = normalizePreferences({ captureDelay: 7, format: 'gif', outputScale: 9, appearance: { from: 'url(secret)' }, presets: [{ name: 'pixels', style: { background: 'image', dataUrl: 'secret' } }] });
  assert.equal(p.captureDelay, 0); assert.equal(p.format, 'png'); assert.equal(p.outputScale, 1); assert.equal(p.presets.length, 0);
  assert.equal(p.appearance.from, '#aa90e5');
  assert.equal(readPreferences({ getItem: () => '{' }).format, 'png');
});
