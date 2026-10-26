import assert from 'node:assert/strict';
import test from 'node:test';
import { DEFAULT_STYLE } from '../src/editor.ts';
import { parseSession, prepareSession, restoreWithBackup, serializeSession, type Session } from '../src/session.ts';

// Original fixed one-pixel fixture; no filesystem or clipboard content is read.
const pixel = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLttAAAAABJRU5ErkJggg==';
function fixture(): Session {
  return { schemaVersion: 1, theme: 'moon', currentId: 'first', documents: [{
    id: 'first', name: '原创示例.png', dataUrl: pixel, width: 1, height: 1,
    state: { annotations: [{ id: 'a', type: 'arrow', x: 1, y: 1, width: -1, height: -1, color: '#abc', lineWidth: 5, points: [{ x: 1, y: 1 }] }], crop: { x: 0, y: 0, width: 1, height: 1 }, style: { ...DEFAULT_STYLE, background: 'image' }, backgroundDataUrl: pixel },
  }] };
}
function rejected(change: (session: any) => void, match: RegExp) {
  const value = fixture(); change(value); assert.throws(() => parseSession(JSON.stringify(value)), match);
}

test('session round trip retains complete editable state and theme without incidental undo history', () => {
  const session = fixture();
  (session.documents[0] as any).undo = [{ secret: 'not persisted' }];
  (session.documents[0].state as any).background = { src: 'runtime image' };
  const saved = serializeSession(session);
  assert.deepEqual(parseSession(saved), fixture());
  assert.ok(!saved.includes('undo') && !saved.includes('runtime image'));
  assert.deepEqual(parseSession(serializeSession({ schemaVersion: 1, theme: 'sakura', currentId: '', documents: [] })), { schemaVersion: 1, theme: 'sakura', currentId: '', documents: [] });
});

test('invalid schema, ids, source links, styles and crops fail before image decoding', () => {
  rejected(s => s.schemaVersion = 2, /会话版本/);
  rejected(s => s.theme = 'injected', /界面外观/);
  rejected(s => s.currentId = 'missing', /当前图片不存在/);
  rejected(s => s.documents.push(s.documents[0]), /编号重复/);
  rejected(s => s.documents[0].dataUrl = 'https://example.com/private.png', /内嵌/);
  rejected(s => s.documents[0].dataUrl = 'data:image/svg+xml;base64,PHN2Zz4=', /内嵌/);
  rejected(s => s.documents[0].dataUrl = 'data:image/png;base64,not-base64', /编码/);
  rejected(s => s.documents[0].state.style.shadow = 1000, /阴影/);
  rejected(s => s.documents[0].state.style.backgroundColor = 'url(https://example.com)', /颜色/);
  rejected(s => s.documents[0].state.crop.x = 1, /裁剪超出/);
  rejected(s => delete s.documents[0].state.backgroundDataUrl, /缺少背景/);
  rejected(s => s.documents[0].state.annotations[0].x = null, /标注横坐标/);
  rejected(s => s.documents[0].state.annotations.push(s.documents[0].state.annotations[0]), /标注编号重复/);
});

test('session count, original pixel budget and UTF-8 name/text budgets are bounded', () => {
  rejected(s => s.documents = Array.from({ length: 7 }, (_, i) => ({ ...s.documents[0], id: `image-${i}` })), /最多保留/);
  rejected(s => { s.documents[0].width = 8000; s.documents[0].height = 6000; }, /像素/);
  rejected(s => { s.documents[0].width = 6000; s.documents[0].height = 5000; s.documents.push({ ...s.documents[0], id: 'second' }); }, /像素/);
  rejected(s => s.documents[0].width = 16385, /原图宽度/);
  rejected(s => s.documents[0].name = '星'.repeat(167), /图片名称/);
  rejected(s => s.documents[0].state.annotations[0].text = '字'.repeat(3334), /标注文字/);
  rejected(s => s.documents[0].state.annotations = Array.from({ length: 2001 }, (_, i) => ({ ...s.documents[0].state.annotations[0], id: `a${i}` })), /标注数量/);
  rejected(s => s.documents[0].state.annotations[0].points = Array.from({ length: 100001 }, () => ({ x: 0, y: 0 })), /标注点数/);
});

test('all original and background pixels are decoded before a prepared session is returned', async () => {
  const session = fixture(); session.documents.push({ ...session.documents[0], id: 'second', state: { ...session.documents[0].state, backgroundDataUrl: undefined, style: { ...DEFAULT_STYLE } } });
  const limits: number[] = [];
  const result = await prepareSession(serializeSession(session), async (_data, limit) => { limits.push(limit); return { width: 1, height: 1 }; });
  assert.deepEqual(limits, [40_000_000, 24_000_000, 40_000_000]);
  assert.equal(result.images.size, 2); assert.equal(result.backgrounds.size, 1);
  await assert.rejects(prepareSession(serializeSession(fixture()), async () => ({ width: 2, height: 1 })), /原图不一致/);
  let calls = 0;
  await assert.rejects(prepareSession(serializeSession(fixture()), async () => (++calls === 1 ? { width: 1, height: 1 } : { width: 6000, height: 5000 })), /背景尺寸超限/);
});

test('restore snapshots current edits before reading/applying old state and aborts on backup or decode failure', async () => {
  const order: string[] = [];
  const actions = {
    async getSession() { order.push('snapshot current'); return 'current'; },
    async backup(session: string) { assert.equal(session, 'current'); order.push('backup current'); return { id: 'backup' }; },
    async read() { order.push('read old'); return serializeSession(fixture()); },
    async apply(session: string) { await prepareSession(session, async () => ({ width: 1, height: 1 })); order.push('replace'); },
  };
  assert.deepEqual(await restoreWithBackup(actions), { id: 'backup' });
  assert.deepEqual(order, ['snapshot current', 'backup current', 'read old', 'replace']);
  order.length = 0;
  await assert.rejects(restoreWithBackup({ ...actions, backup: async () => { throw Error('disk full'); } }), /disk full/);
  assert.deepEqual(order, ['snapshot current']);
  order.length = 0;
  await assert.rejects(restoreWithBackup({ ...actions, read: async () => 'damaged data' }), /JSON/);
  assert.deepEqual(order, ['snapshot current', 'backup current']);
});
