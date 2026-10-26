// Run with Vite and an already authorized, active Ego task space/page:
//   ego-browser nodejs -e 'globalThis.starshotRegressionConfig={spaceId:SPACE_ID,page:"PAGE_LABEL"}; await import("file://REPO_PATH/tests/browser-update-regression.mjs")'
// All native commands are mocked before loading the real App. Only original fixture pixels
// are used; unexpected native operations fail. No real updates, files, desktop or clipboard
// are accessed. This script never creates or finishes a TaskSpace.
const fs = await import('node:fs/promises');
const { fileURLToPath } = await import('node:url');
const { join } = await import('node:path');
const assert = (await import('node:assert/strict')).default;
const config = globalThis.starshotRegressionConfig;
if (!Number.isInteger(config?.spaceId) || config.spaceId <= 0 || typeof config.page !== 'string' || !config.page.trim()) {
  throw Error('Set globalThis.starshotRegressionConfig={spaceId, page} to an already authorized, active Ego task space and its existing page.');
}
const root = fileURLToPath(new URL('../', import.meta.url));
const artifacts = join(root, 'artifacts', 'update-regression');
await fs.mkdir(artifacts, { recursive: true });
const currentVersion = JSON.parse(await fs.readFile(join(root, 'package.json'), 'utf8')).version;
const [major, minor, patch] = currentVersion.split('.').map(Number);
const candidateVersion = `${major}.${minor}.${patch + 1}`;
const page = (await taskSpace(config.spaceId)).page(config.page);
const results = [];
const source = '(' + function ({ currentVersion, candidateVersion }) {
  window.isTauri=true;
  const callbacks = new Map(), listeners = new Map();
  let callbackId = 0, eventId = 0;
  const canvas = document.createElement('canvas'); canvas.width = 640; canvas.height = 360;
  const ctx = canvas.getContext('2d'); ctx.fillStyle = '#304666'; ctx.fillRect(0, 0, 640, 360); ctx.fillStyle = '#b8a1ef'; ctx.fillRect(40, 40, 560, 280);
  const dataUrl = canvas.toDataURL('image/png');
  const style = {background:'solid',backgroundColor:'#223344',gradientFrom:'#b5a0e2',gradientTo:'#617aaa',gradientAngle:135,padding:40,radius:16,borderWidth:2,borderColor:'#ffffff',shadow:16,aspect:'auto'};
  const session = JSON.stringify({schemaVersion:1,theme:'moss',currentId:'fixture-original',documents:[{id:'fixture-original',name:'恢复原创示例.png',dataUrl,width:640,height:360,state:{annotations:[{id:'fixture-annotation',type:'rect',x:40,y:40,width:100,height:80,color:'#ffffff',lineWidth:5}],crop:{x:0,y:0,width:640,height:360},style}}]});
  const original = {id:'before-update-fixture',createdAt:1792972800000,appVersion:'0.1.0',schemaVersion:1,reason:'before-update',totalBytes:session.length,path:'/fixture-only/before-update-fixture.json',compatible:true};
  const mock = window._starshotMock = {calls:[],points:[original],sessions:{[original.id]:session},backupFailure:false,readFailure:false,installation:null,status:{revision:0,currentVersion,configured:true,phase:'idle',downloaded:0}};
  mock.emit = (patch) => { mock.status = {...mock.status,...patch}; for (const listener of listeners.values()) if(listener.event==='starshot-update-status') callbacks.get(listener.handler)({event:listener.event,id:listener.id,payload:{...mock.status}}); };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener(_event,id){listeners.delete(id);}};
  window.__TAURI_INTERNALS__ = {
    transformCallback(callback){const id=++callbackId;callbacks.set(id,callback);return id;},
    async invoke(command,args){
      mock.calls.push({command,args});
      if(command==='plugin:event|listen'){const id=++eventId;listeners.set(id,{...args,id});return id;}
      if(command==='plugin:event|unlisten'){listeners.delete(args.eventId);return;}
      if(command==='get_current_image')return null;
      if(command==='get_app_status')return {captureAvailable:true};
      if(command==='get_update_status')return {...mock.status};
      if(command==='list_recovery_points')return [...mock.points];
      if(command==='check_starshot_update'){mock.emit({revision:mock.status.revision+1,phase:'available',available:{version:candidateVersion,notes:'原创测试更新说明 <img src=x onerror=alert(1)>',date:'2026-10-26T00:00:00Z'}});return {...mock.status};}
      if(command==='create_recovery_point'){
        if(mock.backupFailure)throw Error('原创测试：备份磁盘不可用');
        const point={...original,id:`manual-${mock.points.length}`,reason:'manual',createdAt:Date.now(),appVersion:currentVersion,totalBytes:args.session.length,path:'/fixture-only/manual.json'};
        mock.sessions[point.id]=args.session;mock.points.unshift(point);return point;
      }
      if(command==='read_recovery_point')return {point:mock.points.find(p=>p.id===args.id),session:mock.readFailure?'damaged fixture JSON':mock.sessions[args.id]};
      if(command==='install_starshot_update'){
        mock.installation=args;mock.emit({revision:10,phase:'downloading',downloaded:20,total:100});
        return new Promise(resolve=>{mock.finishInstall=()=>{mock.emit({revision:20,phase:'installed',backupId:'upgrade-test',downloaded:100,total:100});resolve({...mock.status});};});
      }
      throw Error('测试禁止真实原生操作：'+command);
    }
  };
} + ')(' + JSON.stringify({ currentVersion, candidateVersion }) + ');';

// The CDP registration and navigation intentionally share one invocation. Remove the
// preload afterward so a future human navigation cannot inherit the simulated service.
const preload = await page.cdp('Page.addScriptToEvaluateOnNewDocument', { source });
try {
  await page.goto('http://127.0.0.1:1430');
  await page.waitForSelector('loc=role:button[name="可恢复升级前会话"]', { state: 'visible' });
  console.log(await page.snapshot());
  assert.equal(await page.evaluate(() => !document.querySelector('canvas[aria-label="图片编辑画布"]')), true);
  await page.click('loc=role:button[name="用一张原创示例，试试星镜"]');
  await page.waitForFunction(() => document.querySelector('.document-title')?.textContent?.includes('灵感手记'));
  await page.click('loc=role:button[name="设置与帮助"]');
  console.log(await page.snapshot({ scope: 'full_page' }));
  await page.click('loc=role:button[name="备份当前会话"]');
  await page.waitForFunction(() => document.querySelector('.update-notice')?.textContent?.includes('已保存'));
  const saved = await page.evaluate(() => {
    const call = window._starshotMock.calls.find(c => c.command === 'create_recovery_point');
    const s = JSON.parse(call.args.session);
    return { schema: s.schemaVersion, count: s.documents.length, name: s.documents[0].name, width: s.documents[0].width, height: s.documents[0].height, hasUndo: 'undo' in s.documents[0] };
  });
  assert.deepEqual(saved, { schema: 1, count: 1, name: '灵感手记 · 原创示例.png', width: 1200, height: 750, hasUndo: false });
  results.push({ name: 'startup offers recovery without loading images; explicit backup captures original dimensions', pass: true });

  await page.click('loc=css:.recovery-list li:last-child button');
  await page.waitForSelector('loc=role:region[name="确认恢复会话"]', { state: 'visible' });
  await page.keyboard.press('Escape');
  await page.keyboard.press('ControlOrMeta+o');
  const locked = await page.evaluate(async () => {
    const before = document.querySelector('.document-title').textContent;
    document.querySelector('.app-header .primary').click();
    const input = document.querySelector('input[aria-label="导入本地图片"]'), transfer = new DataTransfer();
    transfer.items.add(new File(['original invalid fixture'], '不能覆盖.png', { type: 'image/png' }));
    input.files = transfer.files; input.dispatchEvent(new Event('change', { bubbles: true }));
    await Promise.resolve();
    return { inert: document.querySelector('.workspace').inert, closeDisabled: document.querySelector('button[aria-label="关闭弹窗"]').disabled, dialogOpen: !!document.querySelector('[aria-label="确认恢复会话"]'), same: before === document.querySelector('.document-title').textContent, captured: window._starshotMock.calls.some(c => c.command === 'start_capture') };
  });
  assert.deepEqual(locked, { inert: true, closeDisabled: true, dialogOpen: true, same: true, captured: false });
  await page.click('loc=role:button[name="取消"]');
  assert.equal(await page.evaluate(() => document.querySelector('.workspace').inert), false);
  results.push({ name: 'confirmation freezes editor/import/capture/Escape until cancellation', pass: true });

  await page.evaluate(() => {
    window._starshotMock.backupFailure = true;
    window._readCount = window._starshotMock.calls.filter(c => c.command === 'read_recovery_point').length;
  });
  await page.click('loc=css:.recovery-list li:last-child button');
  await page.click('loc=role:button[name="备份当前会话并恢复"]');
  await page.waitForFunction(() => document.querySelector('.update-error')?.textContent?.includes('备份磁盘不可用'));
  assert.equal(await page.evaluate(() => window._readCount === window._starshotMock.calls.filter(c => c.command === 'read_recovery_point').length && document.querySelector('.document-title').textContent.includes('灵感手记')), true);
  await page.evaluate(() => { window._starshotMock.backupFailure = false; window._starshotMock.readFailure = true; });
  await page.click('loc=role:button[name="备份当前会话并恢复"]');
  await page.waitForFunction(() => document.querySelector('.update-error')?.textContent?.includes('JSON'));
  assert.equal(await page.evaluate(() => document.querySelector('.document-title').textContent.includes('灵感手记')), true);
  await page.evaluate(() => {
    const m = window._starshotMock;
    m.readFailure = false; m.originalSession = m.sessions['before-update-fixture'];
    const s = JSON.parse(m.originalSession); s.documents[0].dataUrl = 'data:image/png;base64,AAAA';
    m.sessions['before-update-fixture'] = JSON.stringify(s);
  });
  await page.click('loc=role:button[name="备份当前会话并恢复"]');
  await page.waitForFunction(() => !!document.querySelector('.update-error') && !document.querySelector('.update-settings').getAttribute('aria-busy')?.includes('true'));
  assert.equal(await page.evaluate(() => document.querySelector('.document-title').textContent.includes('灵感手记')), true);
  await page.evaluate(() => window._starshotMock.sessions['before-update-fixture'] = window._starshotMock.originalSession);
  await page.click('loc=role:button[name="备份当前会话并恢复"]');
  await page.waitForFunction(() => document.querySelector('.update-notice')?.textContent?.includes('已恢复原图'));
  assert.equal(await page.evaluate(() => document.querySelector('.document-title').textContent.includes('恢复原创示例') && document.documentElement.dataset.theme === 'moss' && !document.querySelector('.workspace').inert), true);
  await page.click('loc=role:button[name="备份当前会话"]');
  await page.waitForFunction(() => document.querySelector('.update-notice')?.textContent?.includes('当前会话已保存'));
  assert.equal(await page.evaluate(() => {
    const m = window._starshotMock, actual = JSON.parse(m.calls.filter(c => c.command === 'create_recovery_point').at(-1).args.session), expected = JSON.parse(m.originalSession);
    const canonical = v => Array.isArray(v) ? v.map(canonical) : v && typeof v === 'object' ? Object.fromEntries(Object.keys(v).sort().map(k => [k, canonical(v[k])])) : v;
    return JSON.stringify(canonical(actual)) === JSON.stringify(canonical(expected));
  }), true);
  results.push({ name: 'backup, JSON or pixel decoding failure preserves current edits; successful restore retains every field', pass: true });

  await page.click('loc=role:button[name="检查更新"]');
  await page.waitForSelector('loc=role:button[name="下载并安装"]', { state: 'visible' });
  assert.equal(await page.evaluate(() => document.querySelector('.update-notes').textContent.includes('<img src=x onerror=alert(1)>') && !document.querySelector('.update-notes img')), true);
  await page.click('loc=role:button[name="下载并安装"]');
  await page.waitForSelector('loc=role:region[name="确认安装更新"]', { state: 'visible' });
  assert.equal(await page.evaluate(() => document.querySelector('.workspace').inert), true);
  await page.click('loc=role:button[name="暂不安装"]');
  assert.equal(await page.evaluate(() => document.querySelector('.workspace').inert), false);
  await page.click('loc=role:button[name="下载并安装"]');
  // A later release announcement cannot change the version the user approved.
  await page.evaluate(() => window._starshotMock.emit({ revision: 2, available: { version: '99.0.0', notes: 'later test announcement' } }));
  await page.click(`loc=role:button[name="确认下载、备份并安装 ${candidateVersion}"]`);
  await page.waitForSelector('loc=css:.update-download progress', { state: 'visible' });
  const installation = await page.evaluate(() => {
    const m = window._starshotMock, session = JSON.parse(m.installation.session);
    return { version: m.installation.expectedVersion, theme: session.theme, name: session.documents[0].name, annotations: session.documents[0].state.annotations.length, progress: document.querySelector('progress').value, locked: document.querySelector('.workspace').inert };
  });
  assert.deepEqual(installation, { version: candidateVersion, theme: 'moss', name: '恢复原创示例.png', annotations: 1, progress: 20, locked: true });
  await page.evaluate(() => window._starshotMock.emit({ revision: 2, phase: 'up-to-date', downloaded: 0 }));
  await page.keyboard.press('Escape');
  assert.equal(await page.evaluate(() => !!document.querySelector('.update-download') && document.querySelector('progress').value === 20 && document.querySelector('.workspace').inert && document.querySelector('button[aria-label="关闭弹窗"]').disabled), true);
  await page.evaluate(() => window._starshotMock.emit({ revision: 11, phase: 'downloading', downloaded: 80, total: 100 }));
  await page.waitForFunction(() => document.querySelector('progress')?.value === 80);
  await page.evaluate(() => window._starshotMock.finishInstall());
  await page.waitForSelector('loc=role:button[name="重启星镜"]', { state: 'visible' });
  await page.keyboard.press('Escape');
  assert.deepEqual(await page.evaluate(() => ({ locked: document.querySelector('.workspace').inert, closeDisabled: document.querySelector('button[aria-label="关闭弹窗"]').disabled, nativeCalls: window._starshotMock.calls.filter(c => ['start_capture', 'copy_image', 'save_image', 'restart_app'].includes(c.command)).length })), { locked: true, closeDisabled: true, nativeCalls: 0 });
  results.push({ name: 'plain-text release notes; fixed approved version/session; stale progress ignored; restart state remains frozen', pass: true });
  await fs.writeFile(join(artifacts, 'results.json'), JSON.stringify({ layer: 'real Chromium App with simulated IPC and original pixels', results }, null, 2));
  console.log({ results, artifact: join(artifacts, 'results.json') });
  console.log(await page.snapshot());
} finally {
  await page.cdp('Page.removeScriptToEvaluateOnNewDocument', { identifier: preload.identifier });
}
// The parent owns the shared TaskSpace lifecycle; do not finish it here.
