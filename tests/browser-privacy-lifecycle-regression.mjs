// Original fixtures and mocked IPC only; resume an authorized Ego space/page, never capture the desktop.
const fs=await import('node:fs/promises');
const assert=(await import('node:assert/strict')).default;
const {fileURLToPath}=await import('node:url');
const root=fileURLToPath(new URL('../',import.meta.url));
const config=globalThis.starshotRegressionConfig;
if(!Number.isInteger(config?.spaceId)||!config.page)throw Error('Pass {spaceId,page} for an existing task page');
const page=(await taskSpace(config.spaceId)).page(config.page), results=[];
const version=JSON.parse(await fs.readFile(root+'package.json','utf8')).version;
const source='('+function(version){
  window.isTauri=true;
  const callbacks=new Map(),listeners=new Map();let id=0;
  const m=window._privacyMock={calls:[],state:null,points:[],sessions:{},holdRelease:false,holdOcr:false,backupFailure:false,exitPending:false,status:{revision:0,currentVersion:version,configured:true,phase:'idle',downloadReady:false,confirmationPending:false,downloaded:0}};
  m.emit=(event,payload)=>{for(const [id,l]of listeners)if(l.event===event)callbacks.get(l.handler)({event,id,payload})};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(_event,id){listeners.delete(id)}};
  window.__TAURI_INTERNALS__={transformCallback(fn){callbacks.set(++id,fn);return id},async invoke(command,args={}){
    m.calls.push({command,args});
    if(command==='plugin:event|listen'){listeners.set(++id,args);return id}
    if(command==='plugin:event|unlisten'){listeners.delete(args.eventId);return}
    if(command==='get_current_image')return null;
    if(command==='get_app_status')return {captureAvailable:true};
    if(command==='get_desktop_settings')return {captureShortcut:'CommandOrControl+Shift+2',pinShortcut:'CommandOrControl+Alt+3',startInTray:false,autostart:false};
    if(command==='get_update_status')return {...m.status};
    if(command==='list_recovery_points')return [...m.points];
    if(command==='get_recovery_storage')return {recoveryBytes:123,applicationBytes:456,totalBytes:579};
    if(command==='set_editor_state'){m.state=args;return}
    if(command==='request_exit'){m.exitPending=true;m.emit('request-exit',null);return}
    if(command==='cancel_exit'){m.exitPending=false;return}
    if(command==='confirm_exit'){if(!m.exitPending||m.state.busy)throw Error('Invalid exit handshake');m.confirmed=true;return}
    if(command==='cancel_idle_release')return;
    if(command==='release_empty_editor')return new Promise((_resolve,reject)=>{m.releaseFailure=()=>reject(Error('fixture: preserve WebView for test'))});
    if(command==='recognize_text'){
      const image=new Image();image.src=args.dataUrl;await image.decode();m.ocrSize={width:image.width,height:image.height};
      const result={...m.ocrSize,lines:[{text:'Phone: 13800000000',x:100,y:100,width:280,height:32},{text:'demo@example.invalid',x:100,y:180,width:280,height:32},{text:'password: fixture-only',x:100,y:260,width:280,height:32}]};
      if(m.holdOcr)return new Promise(resolve=>{m.finishOcr=()=>resolve(result)});return result;
    }
    if(command==='save_image'){m.exported=args.dataUrl;return '/fixture-only/saved.png'}
    if(command==='create_recovery_point'){if(m.backupFailure)throw Error('fixture: backup unavailable');const point={id:'fixture-'+m.points.length,createdAt:1793027440000,appVersion:version,schemaVersion:1,reason:'manual',totalBytes:args.session.length,diskBytes:args.session.length,applicationBytes:0,path:'/fixture-only',compatible:true};m.points.unshift(point);m.sessions[point.id]=args.session;return point}
    if(command==='delete_recovery_point'){m.points=m.points.filter(p=>p.id!==args.id);return}
    throw Error('Unexpected mock command: '+command);
  }};
}+')('+JSON.stringify(version)+')';
const preload=await page.cdp('Page.addScriptToEvaluateOnNewDocument',{source});
const wait=condition=>page.waitForFunction(condition,undefined,{timeout:15000});
const click=text=>page.evaluate(text=>{const button=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===text);if(!button)throw Error('Missing '+text);button.click()},text);
try{
  await page.goto('http://127.0.0.1:1430');
  await wait(()=>window._privacyMock?.state&&!window._privacyMock.state.busy);
  await page.evaluate(()=>window._privacyMock.emit('request-idle-release',{requestId:1}));
  await wait(()=>!!window._privacyMock.releaseFailure);
  assert.equal(await page.evaluate(()=>document.querySelector('.workspace').inert),true);
  await click('用一张原创示例，试试星镜');
  assert.equal(await page.evaluate(()=>!!document.querySelector('canvas[aria-label="图片编辑画布"]')),false);
  await page.evaluate(()=>window._privacyMock.releaseFailure());
  await wait(()=>!document.querySelector('.workspace').inert);
  results.push({name:'empty-window release locks input before native acknowledgement and unlocks on failure',pass:true});
  await click('用一张原创示例，试试星镜');
  await wait(()=>window._privacyMock.state?.hasSession&&window._privacyMock.state.dirty&&!window._privacyMock.state.busy);
  await page.evaluate(()=>{window._privacyMock.exitPending=true;window._privacyMock.emit('request-exit',null)});
  await wait(()=>document.querySelector('.exit-dialog').open);
  assert.equal(await page.evaluate(()=>document.querySelector('.workspace').inert),true);
  await page.evaluate(()=>document.querySelector('.exit-dialog').dispatchEvent(new Event('cancel',{cancelable:true})));
  await wait(()=>!document.querySelector('.exit-dialog').open);
  assert.equal(await page.evaluate(()=>window._privacyMock.confirmed||false),false);
  results.push({name:'dirty exit presents backup/discard/cancel and cancel preserves current document',pass:true});
  await click('标注工具');
  await page.evaluate(()=>window._privacyMock.holdOcr=true);
  await click('识别敏感信息');
  await wait(()=>!!window._privacyMock.finishOcr);
  assert.equal(await page.evaluate(()=>window._privacyMock.state.busy),true);
  await page.evaluate(()=>{window._privacyMock.exitPending=true;window._privacyMock.emit('request-exit',null)});
  await wait(()=>!window._privacyMock.exitPending);
  await page.evaluate(()=>window._privacyMock.finishOcr());
  await wait(()=>document.querySelectorAll('.privacy-candidates input').length===3);
  assert.deepEqual(await page.evaluate(()=>window._privacyMock.ocrSize),{width:1200,height:750});
  await page.evaluate(()=>document.querySelectorAll('.privacy-candidates input')[1].click());
  await click('遮盖选中项（可撤销）');
  await wait(()=>!document.querySelector('.privacy-candidates'));
  await click('导出图片');
  await wait(()=>!!window._privacyMock.exported&&!window._privacyMock.state.busy);
  const pixels=await page.evaluate(async()=>{
    const img=new Image();img.src=window._privacyMock.exported;await img.decode();const c=document.createElement('canvas');c.width=img.width;c.height=img.height;const ctx=c.getContext('2d');ctx.drawImage(img,0,0);
    const a=[...ctx.getImageData(72+150,72+115,1,1).data], b=[...ctx.getImageData(72+150,72+195,1,1).data],d=[...ctx.getImageData(72+150,72+275,1,1).data];return {a,b,d};
  });
  assert.deepEqual(pixels.a,[0,0,0,255]);assert.deepEqual(pixels.d,[0,0,0,255]);assert.notDeepEqual(pixels.b,[0,0,0,255]);
  results.push({name:'OCR scans visible editing pixels without padding; chosen whole-line regions export opaque and unselected region remains',pass:true});
  await page.evaluate(()=>document.querySelector('button[aria-label="设置与帮助"]').click());
  await click('备份当前会话');
  await wait(()=>window._privacyMock.points.length===1&&!window._privacyMock.state.busy);
  const data=await page.evaluate(()=>JSON.parse(window._privacyMock.sessions['fixture-0']));
  assert.equal(data.documents[0].state.annotations.filter(a=>a.type==='redact').length,2);
  assert.equal(await page.evaluate(()=>window._privacyMock.state.dirty),false);
  await click('删除备份');await click('确认删除备份');
  await wait(()=>window._privacyMock.points.length===0&&!window._privacyMock.state.busy);
  assert.equal(await page.evaluate(()=>document.querySelector('.document-title').textContent.includes('灵感手记')),true);
  results.push({name:'explicit backup includes redaction geometry; deleting backup leaves live editing intact',pass:true});
  await page.evaluate(()=>document.querySelector('button[aria-label="关闭弹窗"]').click());
  await page.evaluate(()=>document.querySelector('button[aria-label="撤销"]').click());
  await wait(()=>window._privacyMock.state.dirty);
  await page.evaluate(()=>{window._privacyMock.backupFailure=true;window._privacyMock.exitPending=true;window._privacyMock.emit('request-exit',null)});
  await wait(()=>document.querySelector('.exit-dialog').open);
  await click('备份并退出');
  await wait(()=>document.querySelector('.exit-dialog [role="alert"]')?.textContent.includes('unavailable'));
  assert.equal(await page.evaluate(()=>window._privacyMock.confirmed||false),false);
  await page.evaluate(()=>window._privacyMock.backupFailure=false);
  await click('备份并退出');await wait(()=>window._privacyMock.confirmed);
  assert.equal(await page.evaluate(()=>window._privacyMock.state.busy),false);
  results.push({name:'failed exit backup never confirms exit; retry saves current state then flushes idle state before confirmation',pass:true});
  await fs.mkdir(root+'artifacts/privacy-lifecycle',{recursive:true});
  await fs.writeFile(root+'artifacts/privacy-lifecycle/results.json',JSON.stringify({version,results},null,2));
  console.log(JSON.stringify({version,results},null,2));
}finally{await page.cdp('Page.removeScriptToEvaluateOnNewDocument',{identifier:preload.identifier});}
