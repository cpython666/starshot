// Existing authorized Ego TaskSpace only; original demo + generated pixels, mocked native IPC.
// No real screen or clipboard access. Capture UI, Canvas, worker and downloads are real.
const fs = await import('node:fs/promises');
const assert = (await import('node:assert/strict')).default;
const { fileURLToPath } = await import('node:url');
const root = fileURLToPath(new URL('../', import.meta.url));
const config = globalThis.starshotRegressionConfig;
if (!Number.isInteger(config?.spaceId) || !config.page) throw Error('Pass an existing authorized {spaceId,page}');
const page = (await taskSpace(config.spaceId)).page(config.page), results = [];
const dir = root + 'artifacts/capture-toolbar'; await fs.mkdir(dir, { recursive: true });
const wait = fn => page.waitForFunction(fn, undefined, { timeout: 15_000 });
const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const click = label => page.evaluate(label => { const b = document.querySelector(`button[aria-label="${label}"]`); if (!b || b.disabled) throw Error('Unavailable '+label); b.click(); }, label);
const state = () => page.evaluate(() => {
  const s = document.querySelector('.capture-stage').__vueParentComponent.setupState;
  return JSON.parse(JSON.stringify({ selected:s.selected, hovered:s.hovered, annotations:s.annotations, error:s.error, busy:s.busy, tool:s.tool, status:s.status }));
});
const gesture = (start, end = start, shift = false) => page.evaluate(async ({start,end,shift}) => {
  const stage = document.querySelector('.capture-stage'), s = stage.__vueParentComponent.setupState;
  const selection = document.querySelector('.capture-selection');
  const target = selection && (s.selected || s.hovered) ? selection : stage;
  const event = (type,p,buttons) => new PointerEvent(type,{bubbles:true,pointerId:711,pointerType:'mouse',button:0,buttons,shiftKey:shift,clientX:(p.x+.2)/s.frame.width*innerWidth,clientY:(p.y+.2)/s.frame.height*innerHeight});
  const capture=stage.setPointerCapture, has=stage.hasPointerCapture;
  stage.setPointerCapture=()=>{}; stage.hasPointerCapture=()=>false;
  try { target.dispatchEvent(event('pointerdown',start,1)); stage.dispatchEvent(event('pointermove',end,1)); await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))); stage.dispatchEvent(event('pointerup',end,0)); await Promise.resolve(); }
  finally {stage.setPointerCapture=capture;stage.hasPointerCapture=has;}
}, {start,end,shift});
const key = (key, extra={}) => page.evaluate(({key,extra})=>window.dispatchEvent(new KeyboardEvent('keydown',{key,bubbles:true,cancelable:true,...extra})),{key,extra});

await page.goto('http://127.0.0.1:1430/?view=capture');
await wait(()=>document.querySelector('.capture-stage')?.__vueParentComponent.setupState.hovered?.width===560);
assert.equal(await page.evaluate(()=>document.querySelectorAll('.capture-guides i').length),2);
await key('h'); assert.equal(await page.evaluate(()=>!!document.querySelector('.capture-guides')),false);
await key('h'); assert.equal(await page.evaluate(()=>document.querySelectorAll('.capture-guides i').length),2);
const guidePosition=await page.evaluate(async()=>{
  document.querySelector('.capture-stage').dispatchEvent(new PointerEvent('pointermove',{bubbles:true,clientX:250,clientY:180}));
  await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
  return {x:document.querySelector('.guide-vertical').style.left,y:document.querySelector('.guide-horizontal').style.top,events:getComputedStyle(document.querySelector('.capture-guides')).pointerEvents};
});
assert.deepEqual(guidePosition,{x:'250px',y:'180px',events:'none'});
await page.evaluate(async()=>{
  const s=document.querySelector('.capture-stage').__vueParentComponent.setupState;
  document.querySelector('.capture-stage').dispatchEvent(new PointerEvent('pointermove',{bubbles:true,clientX:600/s.frame.width*innerWidth,clientY:375/s.frame.height*innerHeight}));
  await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
});
results.push({name:'horizontal/vertical guides follow the pointer, H toggles them, and neither intercepts selection',pass:true});
const firstCandidate=(await state()).hovered;
await key('Tab'); const nextCandidate=(await state()).hovered;
assert.notDeepEqual(nextCandidate,firstCandidate);
await key('Tab',{shiftKey:true}); assert.deepEqual((await state()).hovered,firstCandidate);
await key('Tab'); await gesture({x:600,y:375});
assert.deepEqual((await state()).selected,{x:nextCandidate.x,y:nextCandidate.y,width:nextCandidate.width,height:nextCandidate.height});
await click('重选区域'); await settle();
assert.deepEqual((await state()).hovered,firstCandidate);
results.push({name:'Tab expands along the enclosing path, Shift+Tab returns inward, and a stationary click keeps the keyboard choice',pass:true});
await gesture({x:600,y:375});
assert.deepEqual((await state()).selected,{x:266,y:239,width:560,height:320});
assert.equal(await page.evaluate(()=>!!document.querySelector('canvas[aria-label="图片编辑画布"]')),false);
assert.equal(await page.evaluate(()=>document.querySelectorAll('.capture-tools button').length),17);
assert.equal(await page.evaluate(()=>!!document.querySelector('.capture-guides')),false);
assert.equal(await page.evaluate(()=>document.querySelector('button[aria-label="滚动长截图"]').disabled),true);
results.push({name:'real worker identifies the original rounded card; selection shows inline tools without the main editor',pass:true});
await click('实心遮盖'); await gesture({x:330,y:300}); await settle();
assert.equal((await state()).error,''); assert.equal((await state()).annotations.length,0);
await gesture({x:330,y:300},{x:460,y:350}); await settle();
assert.equal((await state()).annotations[0].type,'redact');
await click('箭头'); await gesture({x:350,y:320},{x:430,y:330}); await settle();
await click('撤销'); assert.equal((await state()).annotations.length,1);
await click('重做'); assert.equal((await state()).annotations.length,2);
const downloadPromise=page.waitForEvent('download',{timeout:30_000});
await click('保存截图'); const download=await downloadPromise; await download.saveAs(dir+'/annotated.png');
await wait(()=>document.querySelector('.capture-status')?.textContent.includes('已发起 PNG 下载'));
const exported = await fs.readFile(dir+'/annotated.png');
const pixels=await page.evaluate(async data=>{const img=new Image();img.src=data;await img.decode();const c=document.createElement('canvas');c.width=img.width;c.height=img.height;const ctx=c.getContext('2d');ctx.drawImage(img,0,0);return {width:c.width,height:c.height,pixel:[...ctx.getImageData(350-266,320-239,1,1).data]};},'data:image/png;base64,'+exported.toString('base64'));
assert.deepEqual(pixels,{width:560,height:320,pixel:[0,0,0,255]});
results.push({name:'inline redact/arrow/undo/redo produce an actual exact-size PNG with opaque masking; zero-area click is harmless',pass:true,...pixels});
await click('文字');
await page.evaluate(()=>{const el=document.querySelector('input[aria-label="截图标注文字"]');el.value='原创标注';el.dispatchEvent(new Event('input',{bubbles:true}));});
await gesture({x:500,y:470}); await settle();
assert.equal((await state()).annotations.at(-1).text,'原创标注');
await page.screenshot({path:dir+'/inline-toolbar.png'});
await click('更多编辑'); await wait(()=>!!document.querySelector('canvas[aria-label="图片编辑画布"]'));
assert.equal(await page.evaluate(()=>!!document.querySelector('.capture-stage')),false);
results.push({name:'text is placed inline; the full editor opens only on explicit more-editing',pass:true});

const source='('+function(){
  const m=window._captureMock={calls:[],mode:'cancel',hold:false,scrollFrames:1};
  window.__TAURI_INTERNALS__={async invoke(command,args={}){
    m.calls.push({command,args:JSON.parse(JSON.stringify(args))});
    if(command==='get_capture_frame'){
      const c=document.createElement('canvas');c.width=960;c.height=640;
      const ctx=c.getContext('2d');ctx.fillStyle='#eaf0f6';ctx.fillRect(0,0,960,640);
      ctx.fillStyle='#16324f';ctx.fillRect(100,100,400,240);
      return {dataUrl:c.toDataURL('image/png'),width:960,height:640,scaleFactor:1,regions:[]};
    }
    if(command==='finish_capture'){
      if(m.hold)return new Promise(resolve=>{m.release=()=>resolve({completed:false});});
      if(m.mode==='error')throw Error('fixture: clipboard busy');
      return {completed:m.mode==='success',path:m.mode==='success'&&args.action==='save'?'/fixture-only/saved.png':undefined};
    }
    if(command==='cancel_capture')return;
    if(command==='begin_scroll_capture'||command==='append_scroll_capture'){
      if(m.mode==='error')throw Error('fixture: overlap not found');
      if(command==='append_scroll_capture'&&m.mode!=='duplicate')m.scrollFrames++;
      const c=document.createElement('canvas');c.width=300;c.height=180+(m.scrollFrames-1)*90;
      const ctx=c.getContext('2d');ctx.fillStyle='#eaf0f6';ctx.fillRect(0,0,c.width,c.height);
      ctx.fillStyle='#16324f';for(let i=0;i<m.scrollFrames;i++)ctx.fillRect(20,20+i*90,260,55);
      return {width:c.width,height:c.height,frames:m.scrollFrames,previewDataUrl:c.toDataURL('image/png'),message:m.mode==='duplicate'?'画面未变化，已保留当前长截图。':'已采集，请继续滚动或完成。'};
    }
    if(command==='finish_scroll_capture'){
      if(m.mode==='error')throw Error('fixture: save failed');
      return {completed:m.mode==='success'};
    }
    throw Error('Unexpected native command: '+command);
  }};
}+')()';
const preload=await page.cdp('Page.addScriptToEvaluateOnNewDocument',{source});
try {
  await page.goto('http://127.0.0.1:1430/?view=capture');
  await wait(()=>document.querySelector('.capture-stage')?.__vueParentComponent.setupState.ready);
  await gesture({x:150,y:130},{x:449,y:309},true);
  assert.deepEqual((await state()).selected,{x:150,y:130,width:300,height:180});
  await click('实心遮盖'); await gesture({x:170,y:150},{x:220,y:180});
  await click('保存截图'); await wait(()=>document.querySelector('.capture-status')?.textContent.includes('已取消保存'));
  assert.equal((await state()).annotations.length,1);
  assert.deepEqual(await page.evaluate(()=>window._captureMock.calls.at(-1).args.rect),{x:150,y:130,width:300,height:180});
  const nativePixels=await page.evaluate(async()=>{const args=window._captureMock.calls.at(-1).args;const img=new Image();img.src=args.dataUrl;await img.decode();const c=document.createElement('canvas');c.width=img.width;c.height=img.height;const ctx=c.getContext('2d');ctx.drawImage(img,0,0);return {action:args.action,width:c.width,height:c.height,pixel:[...ctx.getImageData(30,30,1,1).data]};});
  assert.deepEqual(nativePixels,{action:'save',width:300,height:180,pixel:[0,0,0,255]});
  await page.evaluate(()=>{window._captureMock.mode='error'});await click('复制截图');await wait(()=>document.querySelector('[role=alert]')?.textContent.includes('clipboard busy'));
  assert.equal((await state()).annotations.length,1);
  results.push({name:'mock native save cancellation and copy failure preserve exact crop and annotations; edited PNG has only cropped opaque pixels',pass:true});
  await page.evaluate(()=>{window._captureMock.mode='success';window._captureMock.hold=true});
  await click('复制截图');await wait(()=>!!window._captureMock.release);
  const before=await page.evaluate(()=>window._captureMock.calls.length);
  await key('Escape');await key('s',{ctrlKey:true});
  assert.equal(await page.evaluate(()=>window._captureMock.calls.length),before);
  assert.equal(await page.evaluate(()=>[...document.querySelectorAll('.capture-tools button')].every(b=>b.disabled)),true);
  await page.evaluate(()=>{window._captureMock.hold=false;window._captureMock.release()});await wait(()=>!document.querySelector('.capture-stage').__vueParentComponent.setupState.busy);
  await click('重选区域');await gesture({x:200,y:200},{x:499,y:399},true);
  await key('Enter');await wait(()=>window._captureMock.calls.at(-1)?.args.action==='copy');
  assert.equal(await page.evaluate(()=>window._captureMock.calls.at(-1).args.dataUrl??null),null);
  await click('贴到桌面');await wait(()=>window._captureMock.calls.at(-1)?.args.action==='pin');
  await click('更多编辑');await wait(()=>window._captureMock.calls.at(-1)?.args.action==='edit');
  results.push({name:'busy gate blocks duplicate copy/save/cancel; Enter copies and unannotated native actions avoid image roundtrip',pass:true});
  // Coalesced moves cannot schedule an unbounded per-event magnifier read.
  await click('重选区域');
  const reads=await page.evaluate(async()=>{
    const stage=document.querySelector('.capture-stage');const original=CanvasRenderingContext2D.prototype.getImageData;let count=0;
    CanvasRenderingContext2D.prototype.getImageData=function(...args){count++;return original.apply(this,args)};
    try {for(let i=0;i<200;i++)stage.dispatchEvent(new PointerEvent('pointermove',{bubbles:true,clientX:200+i/2,clientY:180+i/2}));await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return count;}
    finally {CanvasRenderingContext2D.prototype.getImageData=original;}
  });
  assert.ok(reads<=2,reads);
  results.push({name:'200 queued pointer moves coalesce into at most two tiny magnifier reads',pass:true,reads});
  await gesture({x:150,y:130},{x:449,y:309},true);
  await click('实心遮盖');await gesture({x:170,y:150},{x:220,y:180});
  assert.equal(await page.evaluate(()=>document.querySelector('button[aria-label="滚动长截图"]').disabled),true);
  await click('撤销');await click('调整选区');
  await page.evaluate(()=>{window._captureMock.mode='error'});
  await click('滚动长截图');await wait(()=>document.querySelector('[role=alert]')?.textContent.includes('overlap not found'));
  assert.deepEqual((await state()).selected,{x:150,y:130,width:300,height:180});
  await page.evaluate(()=>{window._captureMock.mode='success'});
  await click('滚动长截图');await wait(()=>!!document.querySelector('.scroll-preview img'));
  assert.equal(await page.evaluate(()=>!!document.querySelector('.capture-stage')),false);
  await page.cdp('Emulation.setDeviceMetricsOverride',{width:380,height:300,deviceScaleFactor:1,mobile:false});
  const panelButton = async text => page.evaluate(text=>{const b=[...document.querySelectorAll('.scroll-capture button')].find(b=>b.textContent.trim()===text);if(!b||b.disabled)throw Error('Unavailable '+text);b.click()},text);
  await panelButton('采集下一屏');await wait(()=>document.querySelector('.scroll-preview').textContent.includes('270'));
  await page.evaluate(()=>{window._captureMock.mode='duplicate'});await panelButton('采集下一屏');
  await wait(()=>document.querySelector('.scroll-status').textContent.includes('未变化'));
  assert.ok(await page.evaluate(()=>document.querySelector('.scroll-preview').textContent.includes('已采集 2 屏')));
  await page.evaluate(()=>{window._captureMock.mode='error'});await panelButton('采集下一屏');
  await wait(()=>document.querySelector('[role=alert]')?.textContent.includes('overlap not found'));
  assert.ok(await page.evaluate(()=>document.querySelector('.scroll-preview').textContent.includes('270')));
  await page.evaluate(()=>{window._captureMock.mode='cancel'});await panelButton('保存');
  await wait(()=>document.querySelector('.scroll-status').textContent.includes('已取消保存'));
  await page.screenshot({path:dir+'/scroll-panel.png'});
  assert.equal(await page.evaluate(()=>{const p=document.querySelector('.scroll-capture');return p.scrollHeight>p.clientHeight}),false);
  await page.evaluate(()=>{window._captureMock.mode='success'});
  for(const [label,action] of [['更多编辑','edit'],['贴到桌面','pin'],['复制完成','copy']]){
    await panelButton(label);await wait(()=>!document.querySelector('.scroll-capture').getAttribute('aria-busy')||document.querySelector('.scroll-capture').getAttribute('aria-busy')==='false');
    assert.equal(await page.evaluate(()=>window._captureMock.calls.at(-1).args.action),action);
  }
  await key('Escape');assert.equal(await page.evaluate(()=>window._captureMock.calls.at(-1).command),'cancel_capture');
  results.push({name:'mock native scroll lifecycle: start rollback, annotation guard, append preview, duplicate/no-overlap preservation, save cancel, actions and Esc; 380x300 panel fits',pass:true});
  await page.cdp('Emulation.clearDeviceMetricsOverride');
} finally {await page.cdp('Page.removeScriptToEvaluateOnNewDocument',{identifier:preload.identifier});}
await fs.writeFile(dir+'/results.json',JSON.stringify({layer:'real Chromium Vue/Canvas/worker plus mocked native command routing; no OS capture or clipboard',results},null,2));
console.log({results});
