import assert from 'node:assert/strict';
import test from 'node:test';
import { DEFAULT_STYLE } from '../src/editor.ts';
import { parseSession, prepareSession, restoreWithBackup, serializeSession, type Session } from '../src/session.ts';

// Original fixed one-pixel fixture; no filesystem or clipboard content is read.
const pixel = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLttAAAAABJRU5ErkJggg==';
function fixture(): Session {
  return { schemaVersion: 2, theme: 'moon', currentId: 'first', documents: [{
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
  assert.deepEqual(parseSession(serializeSession({ schemaVersion: 2, theme: 'sakura', currentId: '', documents: [] })), { schemaVersion: 2, theme: 'sakura', currentId: '', documents: [] });
});

test('internal backups omit only finite empty redactions across all documents without changing editor history', () => {
  const session = fixture();
  const base = { type: 'redact' as const, x: .2, y: .3, width: .01, height: .01, color: '#000000', lineWidth: 1 };
  session.documents.push({ ...session.documents[0]!, id: 'other', state: { ...session.documents[0]!.state, annotations: [
    { ...base, id: 'zero-width', width: 0 },
    { ...base, id: 'zero-height', height: 0 },
    { ...base, id: 'zero-both', width: 0, height: 0 },
    { ...base, id: 'small-valid' },
    { ...base, id: 'text', type: 'text', width: 0, height: 0, text: '保留文字' },
  ] } });
  const original = structuredClone(session), originalAnnotations = session.documents[1]!.state.annotations;
  const restored = parseSession(serializeSession(session));
  assert.deepEqual(session, original);
  assert.equal(session.documents[1]!.state.annotations, originalAnnotations);
  assert.deepEqual(restored.documents[0], session.documents[0], 'the current document and negative arrow direction survive');
  assert.deepEqual(restored.documents[1]!.state.annotations, originalAnnotations.slice(3), 'small masks and zero-size text survive');
  assert.throws(() => parseSession(JSON.stringify(session)), /尺寸必须大于零/, 'external restore data stays strict');
});

test('internal empty-mask repair preserves malformed object and original count limits', () => {
  const base = { id: 'mask', type: 'redact' as const, x: 0, y: 0, width: 0, height: 1, color: '#000000', lineWidth: 1 };
  for (const change of [{ width: -1 }, { height: -1 }, { x: NaN }, { y: Infinity }, { width: NaN }, { height: Infinity }]) {
    const session = fixture(); session.documents[0]!.state.annotations = [{ ...base, ...change }];
    assert.throws(() => serializeSession(session), /恢复点内容无效/);
  }
  for (const change of [{ width: 0 }, { dataUrl: 'data:image/png;base64,AAAA' }]) {
    const session = fixture(); session.documents[0]!.state.annotations = [{ ...base, type: 'sticker', width: 1, dataUrl: pixel, ...change }];
    assert.throws(() => serializeSession(session), /贴纸/);
  }
  const session = fixture();
  session.documents[0]!.state.annotations = Array.from({ length: 2001 }, (_, i) => ({ ...base, id: `empty-${i}` }));
  assert.throws(() => serializeSession(session), /标注数量/);
  session.documents[0]!.state.annotations = [{ ...base, points: Array.from({ length: 100001 }, () => ({ x: 0, y: 0 })) }];
  assert.throws(() => serializeSession(session), /标注点数过多/);
});

test('invalid schema, ids, source links, styles and crops fail before image decoding', () => {
  rejected(s => s.schemaVersion = 3, /会话版本/);
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

test('schema2 retains sticker/redaction objects and decodes each sticker source once before restoring', async () => {
  const session = fixture();
  const base = { x: 0, y: 0, width: 1, height: 1, color: '#123456', lineWidth: 1 };
  session.documents[0]!.state.annotations = [
    { ...base, id: 'sticker-1', type: 'sticker', dataUrl: pixel, rotation: 90 },
    { ...base, id: 'sticker-2', type: 'sticker', dataUrl: pixel },
    { ...base, id: 'mask', type: 'redact' },
  ];
  const serialized = serializeSession(session);
  assert.deepEqual(parseSession(serialized), session);
  const limits: number[] = [];
  const prepared = await prepareSession(serialized, async (_data, limit) => { limits.push(limit); return { width: 1, height: 1 }; });
  assert.deepEqual(limits, [40_000_000, 24_000_000, 2_000_000]);
  assert.equal(prepared.stickers.size, 1);
  assert.deepEqual(prepared.stickers.get(pixel), { width: 1, height: 1 });
  await assert.rejects(prepareSession(serialized, async (_data, limit) => ({ width: limit === 2_000_000 ? 2 : 1, height: 1 })), /贴纸尺寸/);
  await assert.rejects(prepareSession(serialized, async (_data, limit) => { if (limit === 2_000_000) throw Error('PNG decode failed'); return { width: 1, height: 1 }; }), /PNG decode failed/);
});

test('schema1 restores old editable data into schema2 without rewriting the supplied bytes', () => {
  const legacy = JSON.stringify({ ...fixture(), schemaVersion: 1 });
  const prepared = parseSession(legacy);
  assert.deepEqual(prepared, fixture());
  assert.equal(JSON.parse(legacy).schemaVersion, 1);
  assert.equal(JSON.parse(serializeSession(prepared)).schemaVersion, 2);
  rejected(s => {
    s.schemaVersion = 1;
    s.documents[0].state.annotations = [{ id: 'new-object', type: 'redact', x: 0, y: 0, width: 1, height: 1, color: '#000', lineWidth: 1 }];
  }, /旧会话版本/);
});

test('sticker fields and resource budgets are checked across all session documents', () => {
  const sticker = { id: 'sticker', type: 'sticker', x: 0, y: 0, width: 1, height: 1, color: '#000', lineWidth: 1, dataUrl: pixel, rotation: 0 };
  const change = (update: (a: any) => void) => rejected(s => { s.documents[0].state.annotations = [structuredClone(sticker)]; update(s.documents[0].state.annotations[0]); }, /贴纸|遮挡/);
  change(a => delete a.dataUrl);
  change(a => a.dataUrl = 'https://example.com/private.png');
  change(a => a.rotation = -1);
  change(a => a.rotation = 361);
  change(a => a.width = 0);
  change(a => { a.type = 'redact'; a.height = -1; });
  const bytes = Buffer.from(pixel.split(',')[1]!, 'base64');
  bytes.writeUInt32BE(2000, 16); bytes.writeUInt32BE(1000, 20);
  const large = { ...sticker, dataUrl: `data:image/png;base64,${bytes.toString('base64')}` };
  rejected(s => {
    s.documents[0].state.annotations = Array.from({ length: 3 }, (_, i) => ({ ...large, id: `a${i}` }));
    s.documents.push({ ...s.documents[0], id: 'second', state: { ...s.documents[0].state, annotations: [{ ...large, id: 'a3' }, { ...large, id: 'a4' }] } });
  }, /800 万/);
});
