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
  const mock = window._starshotMock = {calls:[],points:[original],sessions:{[original.id]:session},backupFailure:false,readFailure:false,installFailure:false,installation:null,status:{revision:0,currentVersion,configured:true,phase:'idle',downloadReady:false,downloaded:0}};
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
      if(command==='check_starshot_update'){mock.emit({revision:mock.status.revision+1,phase:'available',downloadReady:false,error:null,available:{version:candidateVersion,notes:'原创测试更新说明 <img src=x onerror=alert(1)>',date:'2026-10-26T00:00:00Z'}});return {...mock.status};}
      if(command==='create_recovery_point'){
        if(mock.backupFailure)throw Error('原创测试：备份磁盘不可用');
        const point={...original,id:`manual-${mock.points.length}`,reason:'manual',createdAt:Date.now(),appVersion:currentVersion,totalBytes:args.session.length,path:'/fixture-only/manual.json'};
        mock.sessions[point.id]=args.session;mock.points.unshift(point);return point;
      }
      if(command==='read_recovery_point')return {point:mock.points.find(p=>p.id===args.id),session:mock.readFailure?'damaged fixture JSON':mock.sessions[args.id]};
      if(command==='download_starshot_update'){
        mock.download=args;mock.emit({revision:mock.status.revision+1,phase:'downloading',downloadReady:false,downloaded:20,total:100,error:null});
        return new Promise((resolve,reject)=>{
          mock.readyDownload=()=>mock.emit({revision:mock.status.revision+1,phase:'ready',downloadReady:true,downloaded:100,total:100});
          mock.finishDownload=()=>{if(mock.status.phase!=='ready')mock.readyDownload();resolve({...mock.status});};
          mock.failDownload=()=>{mock.emit({revision:mock.status.revision+1,phase:'error',downloadReady:false,error:'原创测试：下载签名校验失败'});reject(Error(mock.status.error));};
        });
      }
      if(command==='install_starshot_update'){
        if(!mock.status.downloadReady||args.expectedVersion!==mock.status.available?.version)throw Error('测试：没有对应的已验证更新包');
        mock.installation=args;mock.emit({revision:mock.status.revision+1,phase:'backing-up',error:null});
        if(mock.installFailure){mock.emit({revision:mock.status.revision+1,phase:'error',error:'原创测试：安装前备份失败',downloadReady:true});return {...mock.status};}
        return new Promise(resolve=>{
          mock.beginInstall=()=>mock.emit({revision:mock.status.revision+1,phase:'installing'});
          mock.finishInstall=()=>{mock.emit({revision:mock.status.revision+1,phase:'installed',downloadReady:false,backupId:'upgrade-test'});resolve({...mock.status});};
        });
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
  await page.waitForSelector('loc=role:button[name="更新"]', { state: 'visible' });
  assert.equal(await page.evaluate(() => document.querySelector('.update-notes').textContent.includes('<img src=x onerror=alert(1)>') && !document.querySelector('.update-notes img')), true);
  const backupsBeforeDownload = await page.evaluate(() => window._starshotMock.calls.filter(c => c.command === 'create_recovery_point').length);
  await page.click('loc=role:button[name="更新"]');
  await page.waitForSelector('loc=css:.update-download progress', { state: 'visible' });
  assert.deepEqual(await page.evaluate(() => ({ args:window._starshotMock.download, progress:document.querySelector('progress').value, dialog:!!document.querySelector('[data-update-install-dialog][open]'), installation:window._starshotMock.installation, locked:document.querySelector('.workspace').inert })), { args:{expectedVersion:candidateVersion}, progress:20, dialog:false, installation:null, locked:true });
  await page.evaluate(() => window._starshotMock.failDownload());
  await page.waitForFunction(() => !document.querySelector('.workspace').inert);
  assert.equal(await page.evaluate(() => !document.querySelector('[data-update-install-dialog][open]') && document.querySelector('.update-settings').textContent.includes('下载签名校验失败') && !window._starshotMock.installation), true);
  results.push({ name: 'download sends only version and freezes editing; failed verification never offers installation', pass: true });

  await page.click('loc=role:button[name="更新"]');
  await page.waitForSelector('loc=css:.update-download progress', { state: 'visible' });
  await page.evaluate(() => {
    const m=window._starshotMock,latest=m.status;
    m.emit({revision:0,phase:'up-to-date',downloaded:0});m.status=latest;
  });
  await page.keyboard.press('Escape');
  assert.equal(await page.evaluate(() => !!document.querySelector('.update-download') && document.querySelector('progress').value === 20 && document.querySelector('.workspace').inert && document.querySelector('button[aria-label="关闭弹窗"]').disabled), true);
  await page.evaluate(() => {const m=window._starshotMock;m.emit({revision:m.status.revision+1,downloaded:80,total:100,available:{...m.status.available,notes:'新增下载完成后的安装确认，可稍后安装并继续编辑图片。'}});});
  await page.waitForFunction(() => document.querySelector('progress')?.value === 80);
  await page.evaluate(() => window._starshotMock.readyDownload());
  await page.waitForSelector('loc=css:[data-update-install-dialog][open]', { state: 'visible' });
  assert.equal(await page.evaluate(() => document.querySelector('[data-update-install-dialog] .primary').disabled && !window._starshotMock.installation), true);
  await page.evaluate(() => window._starshotMock.finishDownload());
  await page.waitForFunction(() => !document.querySelector('[data-update-install-dialog] .primary').disabled);
  assert.equal(await page.evaluate(() => window._starshotMock.calls.filter(c => c.command === 'create_recovery_point').length), backupsBeforeDownload);
  console.log(await page.snapshot());
  await page.screenshot({ path:join(artifacts,'install-and-restart.png') });
  await page.keyboard.press('Tab');
  assert.equal(await page.evaluate(() => document.querySelector('[data-update-install-dialog]').contains(document.activeElement)), true);
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => !document.querySelector('[data-update-install-dialog][open]') && !document.querySelector('.workspace').inert);
  await page.click('loc=role:button[name="刷新本机状态"]');
  await page.waitForFunction(() => document.querySelector('.update-settings').getAttribute('aria-busy') === 'false');
  assert.equal(await page.evaluate(() => !!document.querySelector('[data-update-install-dialog][open]')), false);
  results.push({ name: 'ready event opens accessible install dialog only after verified download; repeated ready refresh does not reopen after Escape', pass: true });

  await page.click('loc=role:button[name="月白"]');
  await page.click('loc=role:button[name="关闭弹窗"]');
  console.log(await page.snapshot());
  await page.focus('loc=css:input[aria-label="四周留白"]');
  await page.keyboard.press('End');
  await page.waitForFunction(() => document.querySelector('input[aria-label="四周留白"]').value === '200');
  await page.click('loc=role:button[name="设置与帮助"]');
  await page.click('loc=role:button[name="安装并重启"]');
  await page.waitForSelector('loc=css:[data-update-install-dialog][open]', { state: 'visible' });
  await page.click('loc=role:button[name="稍后安装"]');
  await page.waitForFunction(() => !document.querySelector('[data-update-install-dialog][open]') && !document.querySelector('.workspace').inert);
  await page.click('loc=role:button[name="安装并重启"]');
  await page.evaluate(() => {window._starshotMock.installFailure=true;});
  await page.click('loc=css:[data-update-install-dialog][open] .primary');
  await page.waitForFunction(() => document.querySelector('[data-update-install-dialog] [role="alert"]')?.textContent.includes('安装前备份失败') && !document.querySelector('[data-update-install-dialog] .primary').disabled);
  const installation = await page.evaluate(() => {
    const m=window._starshotMock,session=JSON.parse(m.installation.session);
    return {version:m.installation.expectedVersion,theme:session.theme,name:session.documents[0].name,annotations:session.documents[0].state.annotations.length,padding:session.documents[0].state.style.padding,locked:document.querySelector('.workspace').inert,ready:m.status.downloadReady};
  });
  assert.deepEqual(installation,{version:candidateVersion,theme:'moon',name:'恢复原创示例.png',annotations:1,padding:200,locked:true,ready:true});
  assert.equal(await page.evaluate(() => document.querySelector('input[aria-label="四周留白"]').value === '200' && document.documentElement.dataset.theme === 'moon'),true);
  await page.evaluate(() => {window._starshotMock.installFailure=false;});
  await page.click('loc=css:[data-update-install-dialog][open] .primary');
  await page.waitForFunction(() => typeof window._starshotMock.beginInstall === 'function');
  await page.evaluate(() => window._starshotMock.beginInstall());
  await page.keyboard.press('Escape');
  assert.equal(await page.evaluate(() => !!document.querySelector('[data-update-install-dialog][open]') && document.querySelector('[data-update-install-dialog] .primary').disabled && !document.querySelector('.update-download')),true);
  await page.evaluate(() => window._starshotMock.finishInstall());
  await page.waitForSelector('loc=css:[data-update-install-dialog][open] button', { state: 'visible' });
  await page.waitForFunction(() => document.querySelector('#install-update-heading').textContent.includes('正在重启'));
  await page.keyboard.press('Escape');
  assert.deepEqual(await page.evaluate(() => ({locked:document.querySelector('.workspace').inert,closeDisabled:document.querySelector('button[aria-label="关闭弹窗"]').disabled,downloads:window._starshotMock.calls.filter(c => c.command==='download_starshot_update').length,installs:window._starshotMock.calls.filter(c => c.command==='install_starshot_update').length,nativeCalls:window._starshotMock.calls.filter(c => ['start_capture','copy_image','save_image','restart_app'].includes(c.command)).length})),{locked:true,closeDisabled:true,downloads:2,installs:2,nativeCalls:0});
  results.push({ name: 'later resumes editing; install captures newest edits; backup failure retains cached package and retries without downloading; successful install awaits automatic restart', pass: true });
  await fs.writeFile(join(artifacts, 'results.json'), JSON.stringify({ layer: 'real Chromium App with simulated IPC and original pixels', results }, null, 2));
  console.log({ results, artifact: join(artifacts, 'results.json') });
  console.log(await page.snapshot());
} finally {
  await page.cdp('Page.removeScriptToEvaluateOnNewDocument', { identifier: preload.identifier });
}
// The parent owns the shared TaskSpace lifecycle; do not finish it here.
