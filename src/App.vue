<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import Icon from './components/Icon.vue'
const CaptureOverlay = defineAsyncComponent(() => import('./components/CaptureOverlay.vue'))
const PinWindow = defineAsyncComponent(() => import('./components/PinWindow.vue'))
const UpdateSettings = defineAsyncComponent(() => import('./components/UpdateSettings.vue'))
const DesktopSettings = defineAsyncComponent(() => import('./components/DesktopSettings.vue'))
const ShareSettings = defineAsyncComponent(() => import('./components/ShareSettings.vue'))
import PrivacyPanel from './components/PrivacyPanel.vue'
import { readPreferences, normalizePreferences, applyAppearance, PREFERENCES_KEY } from './preferences'
import { findSensitiveRegions, type PrivacyCandidate, type OcrResult } from './privacy'
import { annotationBounds, findAnnotationAt, translateAnnotation, resizeAnnotation, stickerInfo, validateStickerBudget } from './objects'
import { createRecoveryPoint } from './update-api'
import { appVersion, type RecoveryPoint, type UpdateStatus } from './update-api'
import { prepareSession, serializeSession, SESSION_SCHEMA_VERSION, SESSION_THEMES, type SessionTheme } from './session'
import { DEFAULT_STYLE, renderImage, renderPreview, calculateLayout, type Annotation, type EditorStyle } from './editor'
import { normalizeRect, clampRect, type Rect, type Point } from './geometry'

const view = ref(new URLSearchParams(location.search).get('view'))
const desktop = isTauri()
const isMac = navigator.platform.includes('Mac')
const capturePreview = ref(false)
const image = shallowRef<HTMLImageElement | null>(null)
const backgroundImage = shallowRef<HTMLImageElement | undefined>()
const canvas = ref<HTMLCanvasElement>()
const stage = ref<HTMLElement>()
const importInput = ref<HTMLInputElement>()
const backgroundInput = ref<HTMLInputElement>()
const style = ref<EditorStyle>({ ...DEFAULT_STYLE })
const annotations = ref<Annotation[]>([])
const crop = ref<Rect>()
const tool = ref('cursor')
const color = ref('#f78691')
const lineWidth = ref(5)
const fontSize = ref(32)
const text = ref('值得留住的灵感')
const panel = ref<'beautify'|'annotate'>('beautify')
const modal = ref<'history'|'settings'|null>(null)
const status = ref('图片只在本机处理，不上传。')
const isError = ref(false)
const renderError = ref('')
const busy = ref(false)
const updateBusy = ref(false)
const exitPending = ref(false), releasePending = ref(false), privacyBusy = ref(false), ioPending = ref(0)
const actionsBlocked = computed(() => updateBusy.value || exitPending.value || releasePending.value || privacyBusy.value)
const exitDialog = ref<HTMLDialogElement>(), exitWorking = ref(false), exitError = ref('')
const initialized = ref(false)
const dirty = ref(false), sessionRevision = ref(0)
const privacyCandidates = ref<PrivacyCandidate[]>([]), privacyMessage = ref('')
const selectedId = ref(''), selectedAnnotation = computed(() => annotations.value.find(a => a.id === selectedId.value))
const stickerInput = ref<HTMLInputElement>(), stickerImages = new Map<string, HTMLImageElement>()
const presetName = ref('我的美化')
const storedPreferences = readPreferences(localStorage)
const appearance = ref(storedPreferences.appearance), customPresets = ref(storedPreferences.presets)
const shortcuts = ref({ captureShortcut: 'CommandOrControl+Shift+2', pinShortcut: 'CommandOrControl+Alt+3' })
let moveOriginal: Annotation | undefined
let moveCheckpointed = false
let nativeStateQueue = Promise.resolve()
let nativeReady = false
const recoveryAvailable = ref<RecoveryPoint|null>(null)
const captureBusy = ref(false)
const captureDelay = ref(storedPreferences.captureDelay)
const format = ref<'png'|'jpeg'>(storedPreferences.format)
const outputScale = ref<1|2>(storedPreferences.outputScale)
const exportName = ref('星镜图片'), savedPath = ref('')
const currentId = ref('')
const zoom = ref(1)
const fit = ref(1)
const size = ref({width:1200,height:750})
const draft = ref<Annotation>()
const draftCrop = ref<Rect>()
let origin: Point | undefined
let unlisten: UnlistenFn[] = []
let disposed = false
let loadVersion = 0
let backgroundVersion = 0
let resizeObserver: ResizeObserver | undefined
let renderRequest = 0
let modalTrigger: HTMLElement|null = null
let delayTimer: ReturnType<typeof setTimeout> | undefined
let resolveDelay: (()=>void)|undefined
const savedTheme = localStorage.getItem('starshot-theme')
const theme = ref<string>(SESSION_THEMES.includes(savedTheme as SessionTheme) ? savedTheme! : 'starry')
const themes = [{id:'starry',label:'星夜'},{id:'moon',label:'月白'},{id:'plain',label:'素白'},{id:'moss',label:'青苔'},{id:'sakura',label:'暮樱'}]
const tools = [{id:'cursor',name:'选择对象',key:'V'}, {id:'crop',name:'裁剪',key:'C'}, {id:'arrow',name:'箭头',key:'A'}, {id:'rect',name:'矩形',key:'R'}, {id:'ellipse',name:'椭圆',key:'O'}, {id:'pen',name:'画笔',key:'P'}, {id:'highlighter',name:'荧光笔',key:'H'}, {id:'text',name:'文字',key:'T'}, {id:'number',name:'序号',key:'N'}, {id:'mosaic',name:'马赛克',key:'M'}, {id:'redact',name:'实心遮盖',key:'B'}]
const presets = [
  {name:'暮色星云',from:'#b5a0e2',to:'#617aaa',angle:135},
  {name:'极光漫游',from:'#9ecdc2',to:'#405779',angle:145},
  {name:'落日来信',from:'#f6c5b3',to:'#aa8ebd',angle:130},
  {name:'月光浅滩',from:'#e5e6ea',to:'#c8d1dc',angle:135},
  {name:'深空回声',from:'#343b5c',to:'#101522',angle:140},
  {name:'青柠微风',from:'#e5e9ba',to:'#8bb4a2',angle:135},
]
type Snapshot={annotations:Annotation[];crop?:Rect;style:EditorStyle;background?:HTMLImageElement}
type Document={id:string;name:string;dataUrl:string;width:number;height:number;state:Snapshot;undo:Snapshot[];redo:Snapshot[]}
const documents=shallowRef<Document[]>([])
const undoStack=shallowRef<Snapshot[]>([])
const redoStack=shallowRef<Snapshot[]>([])
const currentName=computed(()=>documents.value.find(d=>d.id===currentId.value)?.name || '未命名图片')
const layout=computed(()=>{try{return calculateLayout(size.value,style.value,crop.value,1)}catch{return null}})
const outputSize=computed(()=>{try{const l=calculateLayout(size.value,style.value,crop.value,outputScale.value);return `${l.width} × ${l.height}`}catch{return '尺寸超限'}})
const displayWidth=computed(()=> (layout.value?.width || 1)*fit.value*zoom.value)
const displayHeight=computed(()=> (layout.value?.height || 1)*fit.value*zoom.value)
const annotationHint=computed(()=>tool.value==='text'?'在图片上点击放置文字':tool.value==='number'?'在图片上点击放置递增序号':tool.value==='crop'?'拖动框选裁剪区域，保留原图，可随时恢复':tool.value==='cursor'?'点击选取标注或贴纸，拖动移动；Delete 删除选中对象':'在图片上按住并拖动绘制')
function feedback(message:string,error=false){status.value=message;isError.value=error}
function snapshot():Snapshot{return {annotations:structuredClone(annotations.value.map(a=>({...a,points:a.points?.map(p=>({...p}))}))),crop:crop.value?{...crop.value}:undefined,style:{...style.value},background:backgroundImage.value}}
function closeModal(){if(!updateBusy.value)modal.value=null}
function setUpdateBusy(value:boolean){
  if(value&&!updateBusy.value){loadVersion++;backgroundVersion++;cancelPointer();cancelDelayIfPending()}
  updateBusy.value=value
}
function receiveUpdateStatus(value:UpdateStatus){
  if(value.confirmationPending||['downloading','ready','backing-up','installing','installed','restart-required'].includes(value.phase))modal.value='settings'
}
function embeddedBackground(background?:HTMLImageElement){
  if(!background)return undefined
  if(/^data:image\/(?:png|jpeg|webp);base64,/.test(background.src))return background.src
  const output=document.createElement('canvas');output.width=background.naturalWidth;output.height=background.naturalHeight
  const context=output.getContext('2d');if(!context)throw Error('无法保存背景图片')
  context.drawImage(background,0,0);return output.toDataURL('image/png')
}
async function getSession(){
  // Capture synchronously while the update/recovery panel holds the editing lock.
  const live=snapshot()
  return serializeSession({schemaVersion:SESSION_SCHEMA_VERSION,theme:theme.value as SessionTheme,currentId:currentId.value,documents:documents.value.map(doc=>{
    const state=doc.id===currentId.value?live:doc.state
    return {id:doc.id,name:doc.name,dataUrl:doc.dataUrl,width:doc.width,height:doc.height,state:{annotations:state.annotations,crop:state.crop,style:state.style,backgroundDataUrl:embeddedBackground(state.background)}}
  })})
}
async function restoreSession(serialized:string){
  const version=loadVersion
  const prepared=await prepareSession(serialized,decode)
  if(disposed||version!==loadVersion)throw Error('会话已变化，请重新选择恢复点')
  const next:Document[]=prepared.session.documents.map(doc=>({...doc,state:{annotations:doc.state.annotations,crop:doc.state.crop,style:doc.state.style,background:prepared.backgrounds.get(doc.id)},undo:[],redo:[]}))
  const current=next.find(doc=>doc.id===prepared.session.currentId)
  // All input and images are checked above; replace the session in one synchronous step.
  loadVersion++;backgroundVersion++;cancelPointer();documents.value=next;currentId.value=prepared.session.currentId
  image.value=current?prepared.images.get(current.id)!:null
  stickerImages.clear();for(const [key,value] of prepared.stickers)stickerImages.set(key,value)
  if(current){size.value={width:current.width,height:current.height};restore(current.state);exportName.value=current.name.replace(/\.[^.]+$/,'')||'星镜图片'}
  else{size.value={width:1200,height:750};annotations.value=[];crop.value=undefined;style.value={...DEFAULT_STYLE};backgroundImage.value=undefined;exportName.value='星镜图片'}
  theme.value=prepared.session.theme;undoStack.value=[];redoStack.value=[];zoom.value=1;tool.value='cursor'
  feedback('已恢复会话；恢复前的当前进度已另存为本机恢复点。')
  await nextTick();updateFit();scheduleRender()
}
function checkpoint(){undoStack.value=[...undoStack.value,snapshot()].slice(-60);redoStack.value=[]}
function restore(state:Snapshot){annotations.value=state.annotations;crop.value=state.crop;style.value={...state.style};backgroundImage.value=state.background}
function canRestoreSnapshot(state:Snapshot){try{validateStickerBudget([...documents.value.flatMap(d=>d.id===currentId.value?[]:d.state.annotations),...state.annotations]);return true}catch(e){feedback(String(e),true);return false}}
function undo(){if(actionsBlocked.value)return;const state=undoStack.value.at(-1);if(!state||!canRestoreSnapshot(state))return;redoStack.value=[...redoStack.value,snapshot()];undoStack.value=undoStack.value.slice(0,-1);restore(state)}
function redo(){if(actionsBlocked.value)return;const state=redoStack.value.at(-1);if(!state||!canRestoreSnapshot(state))return;undoStack.value=[...undoStack.value,snapshot()];redoStack.value=redoStack.value.slice(0,-1);restore(state)}
function setStyle<K extends keyof EditorStyle>(key:K,value:EditorStyle[K]){if(actionsBlocked.value)return;if(key==='background')backgroundVersion++;checkpoint();style.value={...style.value,[key]:value}}
const hasCustomStyle = computed(() => !!backgroundImage.value || (Object.keys(DEFAULT_STYLE) as (keyof EditorStyle)[]).some(key => style.value[key] !== DEFAULT_STYLE[key]))
function resetStyle(){
  if(actionsBlocked.value||!image.value||!hasCustomStyle.value)return
  backgroundVersion++;cancelPointer();checkpoint();backgroundImage.value=undefined;style.value={...DEFAULT_STYLE}
  feedback('已恢复默认美化，裁剪和标注保留；可撤销。')
}
function shortcutLabel(value:string){return value.replace(/CommandOrControl|CmdOrCtrl/g,isMac?'⌘':'Ctrl').replaceAll('+',' + ')}
function usePreset(p:typeof presets[number]){if(actionsBlocked.value)return;backgroundVersion++;checkpoint();style.value={...style.value,background:'gradient',gradientFrom:p.from,gradientTo:p.to,gradientAngle:p.angle}}
function remember(){const doc=documents.value.find(d=>d.id===currentId.value);if(doc){doc.state=snapshot();doc.undo=undoStack.value;doc.redo=redoStack.value}}
async function decode(dataUrl:string,maxPixels=40_000_000){
  const img=new Image();img.src=dataUrl;await img.decode()
  if(!img.width||!img.height||img.width*img.height>maxPixels||Math.max(img.width,img.height)>16384)
    throw Error(`图片过大，请使用 ${maxPixels/10_000} 万像素以内、单边不超过 16384 像素的图片。`)
  return img
}
async function loadImage(payload:{dataUrl:string;name:string;width?:number;height?:number},version=++loadVersion,maxPixels=40_000_000,invalid:()=>string|null=()=>null):Promise<string|null>{
  if(actionsBlocked.value)return '编辑器正在处理其他操作，请稍后重试'
  ioPending.value++
  try{
    const img=await decode(payload.dataUrl,maxPixels)
    const invalidReason=invalid()
    if(invalidReason)return invalidReason
    if(version!==loadVersion||disposed||actionsBlocked.value)return '编辑状态已变化，本次图片未导入'
    remember();cancelPointer()
    const id=crypto.randomUUID()
    image.value=img;size.value={width:img.width,height:img.height};annotations.value=[];crop.value=undefined
    backgroundImage.value=undefined;style.value={...DEFAULT_STYLE};undoStack.value=[];redoStack.value=[]
    currentId.value=id;zoom.value=1;exportName.value=payload.name.replace(/\.[^.]+$/,'')||'星镜图片';tool.value='cursor'
    const doc:Document={id,name:payload.name,dataUrl:payload.dataUrl,width:img.width,height:img.height,state:snapshot(),undo:[],redo:[]}
    documents.value=[doc,...documents.value].slice(0,6)
    let pixels=img.width*img.height
    documents.value=documents.value.filter((d,i)=>i===0 || (pixels+=d.width*d.height)<=48_000_000)
    feedback(`已打开 ${payload.name} · ${img.width} × ${img.height}`)
    await nextTick();updateFit();scheduleRender()
    return null
  }catch(e){const message=String(e);if(version===loadVersion&&!disposed)feedback(message,true);return message}finally{ioPending.value--}
}
async function openDocument(doc:Document){
  if(actionsBlocked.value)return
  const version=++loadVersion
  ioPending.value++
  try{
    const img=await decode(doc.dataUrl)
    if(version!==loadVersion||disposed||actionsBlocked.value)return
    remember();cancelPointer();image.value=img;currentId.value=doc.id;size.value={width:doc.width,height:doc.height}
    restore(doc.state);undoStack.value=doc.undo;redoStack.value=doc.redo;zoom.value=1
    exportName.value=doc.name.replace(/\.[^.]+$/,'');modal.value=null;feedback(`已恢复 ${doc.name} 的编辑`)
    await nextTick();updateFit();scheduleRender()
  }catch(e){if(version===loadVersion&&!disposed)feedback(String(e),true)}finally{ioPending.value--}
}
function fileData(file:File){return new Promise<string>((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result));reader.onerror=()=>reject(Error('读取图片失败'));reader.readAsDataURL(file)})}
async function importFile(file?:File,background=false){
  if(!file||actionsBlocked.value)return
  if(!['image/png','image/jpeg','image/webp'].includes(file.type)){feedback('请导入 PNG、JPEG 或 WebP 图片。',true);return}
  if(file.size>32*1024*1024){feedback('图片文件请小于 32 MB。',true);return}
  // Reserve the intent before reading the file, so slow earlier choices cannot win.
  const version=background?loadVersion:++loadVersion
  const backgroundRequest=background?++backgroundVersion:backgroundVersion
  const documentId=currentId.value
  const isCurrent=()=>!disposed&&!actionsBlocked.value&&version===loadVersion&&(!background||(backgroundRequest===backgroundVersion&&documentId===currentId.value))
  ioPending.value++
  try{
    const dataUrl=await fileData(file)
    if(!isCurrent())return
    if(background){
      const decoded=await decode(dataUrl,24_000_000)
      if(!isCurrent())return
      checkpoint();backgroundImage.value=decoded;style.value={...style.value,background:'image'}
      feedback('已使用本地图片作为背景')
    }else await loadImage({dataUrl,name:file.name},version,24_000_000)
  }catch(e){if(isCurrent())feedback(String(e),true)}finally{ioPending.value--}
}
function onFile(event:Event,background=false){const input=event.target as HTMLInputElement;void importFile(input.files?.[0],background);input.value=''}
function onDrop(event:DragEvent){event.preventDefault();void importFile(event.dataTransfer?.files[0])}
async function loadDemo(){
  if(actionsBlocked.value)return
  const version=++loadVersion
  ioPending.value++
  try{
    const img=new Image();img.src='/demo-workspace.svg';await img.decode()
    if(version!==loadVersion||disposed||actionsBlocked.value)return
    const c=document.createElement('canvas');c.width=img.width;c.height=img.height;c.getContext('2d')!.drawImage(img,0,0)
    await loadImage({dataUrl:c.toDataURL('image/png'),name:'灵感手记 · 原创示例.png'},version)
  }catch(e){if(version===loadVersion&&!disposed)feedback(String(e),true)}finally{ioPending.value--}
}
function updateFit(){if(!stage.value||!layout.value)return;fit.value=Math.min((stage.value.clientWidth-80)/layout.value.width,(stage.value.clientHeight-90)/layout.value.height,1);fit.value=Math.max(.03,fit.value)}
function scheduleRender(){if(!renderRequest)renderRequest=requestAnimationFrame(draw)}
function draw(){
  renderRequest=0
  if(!image.value){renderError.value='';return}
  if(!canvas.value||!layout.value)return
  try{
    const dpr=Math.min(2,window.devicePixelRatio||1)
    const output=renderPreview(image.value,size.value,draft.value&&(draft.value.type!=='redact'||(draft.value.width>0&&draft.value.height>0))?[...annotations.value,draft.value]:annotations.value,style.value,{crop:crop.value,backgroundImage:backgroundImage.value,stickers:stickerImages},{width:Math.max(1,displayWidth.value*dpr),height:Math.max(1,displayHeight.value*dpr)})
    if(canvas.value.width!==output.width)canvas.value.width=output.width
    if(canvas.value.height!==output.height)canvas.value.height=output.height
    const ctx=canvas.value.getContext('2d')!;ctx.clearRect(0,0,output.width,output.height);ctx.drawImage(output,0,0)
    const l=layout.value
    ctx.save();ctx.scale(output.width/l.width,output.height/l.height);drawGuides(ctx)
    if(draftCrop.value){const r=draftCrop.value;ctx.strokeStyle='#c6b2ff';ctx.lineWidth=2/Math.max(.1,fit.value*zoom.value);ctx.setLineDash([8,6]);ctx.strokeRect(l.imageRect.x+r.x-l.crop.x,l.imageRect.y+r.y-l.crop.y,r.width,r.height)}
    ctx.restore()
    renderError.value=''
  }catch(e){renderError.value=e instanceof Error?e.message:String(e)}
}
function point(event:PointerEvent):Point|null{if(!canvas.value||!layout.value)return null;const box=canvas.value.getBoundingClientRect(),l=layout.value;return {x:Math.max(l.crop.x,Math.min(l.crop.x+l.crop.width,(event.clientX-box.left)*l.width/box.width-l.imageRect.x+l.crop.x)),y:Math.max(l.crop.y,Math.min(l.crop.y+l.crop.height,(event.clientY-box.top)*l.height/box.height-l.imageRect.y+l.crop.y))}}
function insideImage(event:PointerEvent){if(!canvas.value||!layout.value)return false;const box=canvas.value.getBoundingClientRect(),l=layout.value;const x=(event.clientX-box.left)*l.width/box.width,y=(event.clientY-box.top)*l.height/box.height;return x>=l.imageRect.x&&x<=l.imageRect.x+l.imageRect.width&&y>=l.imageRect.y&&y<=l.imageRect.y+l.imageRect.height}
function pointerDown(event:PointerEvent){if(actionsBlocked.value||event.button!==0||!insideImage(event))return;const p=point(event);if(!p)return;event.preventDefault();canvas.value?.setPointerCapture(event.pointerId);origin=p;if(tool.value==='cursor'){const found=findAnnotationAt(annotations.value,p);selectedId.value=found?.id||'';moveOriginal=found?{...found,points:found.points?.map(p=>({...p}))}:undefined;moveCheckpointed=false;scheduleRender();return}selectedId.value='';if(tool.value==='crop'){draftCrop.value={...p,width:0,height:0};return}const type=tool.value as Annotation['type'];const annotation:Annotation={id:crypto.randomUUID(),type,...p,width:0,height:0,color:color.value,lineWidth:lineWidth.value,points:[p],text:text.value.slice(0,1000),fontSize:fontSize.value,number:annotations.value.filter(a=>a.type==='number').length+1};if(type==='text'||type==='number'){if(type==='text'&&!text.value.trim()){feedback('先在右侧输入文字。',true);origin=undefined;return}checkpoint();annotations.value=[...annotations.value,annotation];origin=undefined;return}draft.value=annotation}
function pointerMove(event:PointerEvent){if(!origin)return;const p=point(event);if(!p)return;if(moveOriginal){const box=annotationBounds(moveOriginal);const dx=Math.max(-box.x,Math.min(size.value.width-box.x-box.width,p.x-origin.x));const dy=Math.max(-box.y,Math.min(size.value.height-box.y-box.height,p.y-origin.y));if(!dx&&!dy&&!moveCheckpointed)return;if(!moveCheckpointed){checkpoint();moveCheckpointed=true}const moved=translateAnnotation(moveOriginal,dx,dy);annotations.value=annotations.value.map(a=>a.id===moved.id?moved:a);scheduleRender();return;}if(tool.value==='crop'){draftCrop.value=normalizeRect(origin,p);scheduleRender();return}if(!draft.value)return;if(tool.value==='pen'||tool.value==='highlighter')draft.value={...draft.value,points:[...(draft.value.points||[]),p]};else if(tool.value==='arrow')draft.value={...draft.value,width:p.x-origin.x,height:p.y-origin.y};else draft.value={...draft.value,...normalizeRect(origin,p)};scheduleRender()}
function pointerUp(event:PointerEvent){if(!origin)return;pointerMove(event);moveOriginal=undefined;if(draftCrop.value&&draftCrop.value.width>2&&draftCrop.value.height>2){checkpoint();crop.value=clampRect(draftCrop.value,size.value);feedback('已裁剪。点击「恢复原图范围」可还原。')}else if(draft.value){const d=draft.value;if(d.type==='redact'?(d.width>2&&d.height>2):(Math.abs(d.width)+Math.abs(d.height)>2||(d.points?.length||0)>1)){checkpoint();annotations.value=[...annotations.value,d]}}origin=undefined;draft.value=undefined;draftCrop.value=undefined;if(canvas.value?.hasPointerCapture(event.pointerId))canvas.value.releasePointerCapture(event.pointerId);scheduleRender()}
function cancelPointer(){moveOriginal=undefined;origin=undefined;draft.value=undefined;draftCrop.value=undefined;scheduleRender()}
function resetCrop(){if(actionsBlocked.value)return;checkpoint();crop.value=undefined;feedback('已恢复原图范围，标注保留。')}
function removeAnnotations(){if(actionsBlocked.value)return;if(!annotations.value.length)return;checkpoint();annotations.value=[];feedback('已清空标注，可撤销。')}
async function startCapture(){if(captureBusy.value||actionsBlocked.value)return;if(!desktop){capturePreview.value=true;return}captureBusy.value=true;try{if(captureDelay.value){feedback(`${captureDelay.value} 秒后开始截图…`);await new Promise<void>(resolve=>{resolveDelay=resolve;delayTimer=setTimeout(resolve,captureDelay.value*1000)});delayTimer=undefined;resolveDelay=undefined;if(!captureBusy.value)return}await invoke('start_capture');feedback('正在选择截图区域')}catch(e){feedback(String(e),true)}finally{captureBusy.value=false}}
function cancelDelay(){if(delayTimer)clearTimeout(delayTimer);delayTimer=undefined;captureBusy.value=false;resolveDelay?.();resolveDelay=undefined;feedback('已取消延时截图')}
function leaveCapture(){capturePreview.value=false;if(!desktop&&view.value==='capture'){view.value=null;history.replaceState(null,'',location.pathname)}}
async function captured(payload:{dataUrl:string;width:number;height:number;name:string}){if(updateBusy.value){feedback('正在更新或恢复会话，截图未加入当前编辑。',true);return}leaveCapture();await loadImage(payload)}
type PinEditRequest={requestId:number;expiresAt:number;image:{dataUrl:string;width:number;height:number;name:string}}
async function receivePendingPinEdit(){
  if(!initialized.value||disposed)return
  try{
    // The native slot can only be taken once, including when the startup pull
    // and event arrive together. It never becomes a replayable capture image.
    const request=await invoke<PinEditRequest|null>('take_pending_pin_edit')
    if(!request)return
    const deadline=performance.now()+Math.max(0,request.expiresAt-Date.now())
    const invalid=()=>performance.now()>=deadline?'贴图发送已超时，本次图片未导入':null
    let error=invalid()
    if(!error&&(disposed||actionsBlocked.value||busy.value||captureBusy.value||ioPending.value>0))error='编辑器正在处理其他操作，请稍后重试'
    if(!error)error=await loadImage(request.image,++loadVersion,40_000_000,invalid)
    await invoke('finish_pin_edit',{requestId:request.requestId,error})
    if(error&&!disposed)feedback(`${error}；贴图保留`,true)
  }catch(e){if(!disposed)feedback(`接收贴图：${String(e)}`,true)}
}
function output(kind:'png'|'jpeg'=format.value){if(!image.value)throw Error('请先截图或导入图片');return renderImage(image.value,size.value,annotations.value,style.value,{crop:crop.value,scale:outputScale.value,backgroundImage:backgroundImage.value,format:kind,stickers:stickerImages}).toDataURL(kind==='png'?'image/png':'image/jpeg',.94)}
async function exportImage(){if(busy.value||actionsBlocked.value)return;busy.value=true;try{const dataUrl=output();const filename=`${exportName.value.replace(/[\\/:*?"<>|\x00-\x1f]/g,'_').slice(0,100)||'星镜图片'}.${format.value==='png'?'png':'jpg'}`;if(desktop){const path=await invoke<string|null>('save_image',{dataUrl,suggestedName:filename});if(path)savedPath.value=path;feedback(path?`已保存到 ${path}`:'已取消保存，编辑内容保留。')}else{const a=document.createElement('a');a.href=dataUrl;a.download=filename;a.click();feedback(`已发起下载：${filename}`)}}catch(e){feedback(String(e),true)}finally{busy.value=false}}
async function copyImage(){if(busy.value||actionsBlocked.value)return;busy.value=true;try{const dataUrl=output('png');if(desktop)await invoke('copy_image',{dataUrl});else{const blob=await(await fetch(dataUrl)).blob();await navigator.clipboard.write([new ClipboardItem({'image/png':blob})])}feedback('已复制图片到剪贴板')}catch(e){feedback(`复制失败：${String(e)}`,true)}finally{busy.value=false}}
async function pinImage(){if(actionsBlocked.value)return;if(!desktop){feedback('置顶贴图需要桌面版；浏览器中可使用美化与导出。',true);return}try{await invoke('pin_image',{dataUrl:output('png')});feedback('已贴到桌面，按住图片即可拖动。')}catch(e){feedback(String(e),true)}}
function selectTool(id:string){if(actionsBlocked.value)return;tool.value=id;if(id!=='cursor')panel.value='annotate';cancelPointer()}
function onKey(event:KeyboardEvent){if(view.value||capturePreview.value||exitPending.value)return;if(event.key==='Tab'&&modal.value){const focusable=Array.from(document.querySelectorAll<HTMLElement>('.dialog button:not(:disabled),.dialog input:not(:disabled),.dialog select:not(:disabled),.dialog textarea:not(:disabled),.dialog summary')).filter(element=>element.getClientRects().length);const first=focusable[0],last=focusable.at(-1);if(event.shiftKey&&document.activeElement===first){event.preventDefault();last?.focus()}else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first?.focus()}return}if(event.key==='Escape'){if(updateBusy.value){event.preventDefault();return}modal.value=null;cancelPointer();cancelDelayIfPending();return}if(modal.value||actionsBlocked.value){if((event.metaKey||event.ctrlKey)&&['s','o'].includes(event.key.toLowerCase()))event.preventDefault();return}if((event.target as HTMLElement)?.closest('input,textarea,select,[contenteditable]'))return;if((event.key==='Delete'||event.key==='Backspace')&&selectedId.value){event.preventDefault();deleteSelected();return}if(event.metaKey||event.ctrlKey){if(event.key.toLowerCase()==='z'){event.preventDefault();event.shiftKey?redo():undo()}else if(event.key.toLowerCase()==='s'){event.preventDefault();if(image.value)void exportImage()}else if(event.key.toLowerCase()==='o'){event.preventDefault();importInput.value?.click()}return}if(modal.value)return;const selected=tools.find(t=>t.key.toLowerCase()===event.key.toLowerCase());if(selected&&image.value){event.preventDefault();selectTool(selected.id)}}
function cancelDelayIfPending(){if(delayTimer)cancelDelay()}
watch([style,crop],()=>{updateFit();scheduleRender()},{deep:true})
watch(annotations,scheduleRender,{deep:true})
watch([fit,zoom],scheduleRender)
watch(backgroundImage,scheduleRender)
watch(capturePreview,async value=>{if(!value){await nextTick();resizeObserver?.disconnect();if(stage.value)resizeObserver?.observe(stage.value);updateFit();scheduleRender()}})
watch(modal,async(value,previous)=>{if(value&&!previous)modalTrigger=document.activeElement as HTMLElement;await nextTick();if(value)document.querySelector<HTMLElement>('.dialog button')?.focus();else modalTrigger?.focus()})
watch(theme,value=>{document.documentElement.dataset.theme=value;applyAppearance(document.documentElement,appearance.value,['moon','plain','sakura'].includes(value));try{localStorage.setItem('starshot-theme',value)}catch{feedback('外观已应用，本地偏好保存失败。',true)}})
watch(image,async()=>{await nextTick();resizeObserver?.disconnect();if(stage.value){resizeObserver?.observe(stage.value);updateFit();scheduleRender()}})
onMounted(async()=>{document.documentElement.dataset.theme=theme.value;window.addEventListener('keydown',onKey);resizeObserver=new ResizeObserver(()=>{updateFit();scheduleRender()});if(stage.value)resizeObserver.observe(stage.value);if(desktop&&!view.value){try{const off1=await listen<{dataUrl:string;width:number;height:number;name:string}>('capture-complete',event=>void captured(event.payload));const off2=await listen<string>('capture-error',event=>feedback(event.payload,true));const off3=await listen<string>('pin-error',event=>feedback(event.payload,true));const off4=await listen('pin-edit-requested',()=>void receivePendingPinEdit());if(disposed){off1();off2();off3();off4();return}unlisten.push(off1,off2,off3,off4);const initialVersion=loadVersion;const last=await invoke<{dataUrl:string;width:number;height:number;name:string}|null>('get_current_image');if(last&&loadVersion===initialVersion&&!disposed&&!updateBusy.value)await loadImage(last);const state=await invoke<{captureAvailable:boolean;error?:string}>('get_app_status');if(state.error)feedback(state.error,true)}catch(e){feedback(`桌面服务：${String(e)}`,true)}}initialized.value=true;void reportEditorState();if(desktop&&!view.value)void receivePendingPinEdit()})
onBeforeUnmount(()=>{disposed=true;loadVersion++;unlisten.forEach(off=>off());resizeObserver?.disconnect();window.removeEventListener('keydown',onKey);cancelAnimationFrame(renderRequest);cancelDelayIfPending()})

function drawGuides(ctx:CanvasRenderingContext2D){
  const l=layout.value;if(!l)return
  ctx.save();ctx.translate(l.imageRect.x-l.crop.x,l.imageRect.y-l.crop.y);ctx.lineWidth=2/Math.max(.1,fit.value*zoom.value);ctx.setLineDash([6,4])
  if(selectedAnnotation.value){const r=annotationBounds(selectedAnnotation.value);ctx.strokeStyle='#8162d9';ctx.strokeRect(r.x,r.y,r.width,r.height)}
  for(const r of privacyCandidates.value){ctx.strokeStyle=r.selected?'#e4566e':'#8b8b96';ctx.strokeRect(r.x,r.y,r.width,r.height)}
  ctx.restore()
}
function savePreset(){
  if(actionsBlocked.value||!image.value)return
  if(style.value.background==='image'){feedback('美化预设不保存图片，请先选择渐变、纯色或透明背景。',true);return}
  const name=presetName.value.trim().slice(0,30)||'我的美化'
  customPresets.value=[{name,style:{...style.value}},...customPresets.value.filter(p=>p.name!==name)].slice(0,12)
  feedback('已保存美化预设，不包含原图。')
}
function applySavedPreset(index:number){if(actionsBlocked.value)return;const p=customPresets.value[index];if(!p)return;backgroundVersion++;checkpoint();backgroundImage.value=undefined;style.value={...p.style}}
function addWatermark(){
  if(actionsBlocked.value||!image.value||!text.value.trim())return
  const area=crop.value||{x:0,y:0,...size.value},font=Math.min(32,Math.max(12,fontSize.value));const value=text.value.trim().slice(0,100)
  const width=Array.from(value).reduce((sum,c)=>sum+(/[^\x00-\xff]/.test(c)?font:font*.65),0)
  const mark:Annotation={id:crypto.randomUUID(),type:'text',x:Math.max(area.x,area.x+area.width-width-20),y:Math.max(area.y,area.y+area.height-font*1.3-20),width:0,height:0,color:color.value,lineWidth:1,fontSize:font,text:value}
  checkpoint();annotations.value=[...annotations.value,mark];selectedId.value=mark.id;tool.value='cursor'
}
function deleteSelected(){if(actionsBlocked.value||!selectedId.value)return;checkpoint();annotations.value=annotations.value.filter(a=>a.id!==selectedId.value);selectedId.value=''}
function editSelected(changes:Partial<Annotation>){if(actionsBlocked.value||!selectedAnnotation.value)return;checkpoint();annotations.value=annotations.value.map(a=>a.id===selectedId.value?{...a,...changes}:a)}
function scaleSelected(percent:number){
  const a=selectedAnnotation.value;if(!a||actionsBlocked.value||!Number.isFinite(percent))return
  const r=annotationBounds(a),factor=Math.max(.25,Math.min(4,percent/100));const width=Math.max(4,Math.min(size.value.width-r.x,r.width*factor)),height=Math.max(4,Math.min(size.value.height-r.y,r.height*factor))
  checkpoint();annotations.value=annotations.value.map(x=>x.id===a.id?resizeAnnotation(a,{...r,width,height}):x)
}
async function importSticker(event:Event){
  const input=event.target as HTMLInputElement,file=input.files?.[0];input.value='';if(!file||!image.value||actionsBlocked.value)return
  const id=currentId.value,version=loadVersion;ioPending.value++
  try{
    if(file.type!=='image/png'||file.size>2*1024*1024)throw Error('贴纸请使用 2 MiB 以内的透明 PNG。')
    const dataUrl=await fileData(file),info=stickerInfo(dataUrl),decoded=await decode(dataUrl,2_000_000)
    if(disposed||id!==currentId.value||version!==loadVersion||actionsBlocked.value)return
    const area=crop.value||{x:0,y:0,...size.value};const factor=Math.min(1,area.width*.45/info.width,area.height*.45/info.height)
    const annotation:Annotation={id:crypto.randomUUID(),type:'sticker',x:area.x+(area.width-info.width*factor)/2,y:area.y+(area.height-info.height*factor)/2,width:info.width*factor,height:info.height*factor,color:'#000000',lineWidth:1,dataUrl,rotation:0}
    validateStickerBudget([...documents.value.flatMap(d=>d.id===currentId.value?[]:d.state.annotations),...annotations.value,annotation])
    stickerImages.set(dataUrl,decoded);checkpoint();annotations.value=[...annotations.value,annotation];selectedId.value=annotation.id;tool.value='cursor';panel.value='annotate';feedback('已加入本地贴纸，可拖动、缩放和旋转。')
  }catch(e){if(id===currentId.value&&version===loadVersion)feedback(String(e),true)}finally{ioPending.value--}
}
async function scanPrivacy(){
  if(!image.value||actionsBlocked.value||busy.value||!desktop)return
  privacyBusy.value=true;privacyCandidates.value=[];privacyMessage.value='正在本机识别，图片不会上传。'
  const id=currentId.value,revision=sessionRevision.value
  try{
    const area=layout.value!.crop
    const flat=renderImage(image.value,size.value,annotations.value,{...DEFAULT_STYLE,background:'transparent',padding:0,radius:0,borderWidth:0,shadow:0,aspect:'auto'},{crop:area,stickers:stickerImages})
    const result=await invoke<OcrResult>('recognize_text',{dataUrl:flat.toDataURL('image/png')})
    if(disposed||id!==currentId.value||revision!==sessionRevision.value)return
    privacyCandidates.value=findSensitiveRegions(result,{x:area.x,y:area.y})
    privacyMessage.value=privacyCandidates.value.length?`找到 ${privacyCandidates.value.length} 处候选，请核对画布中的虚线框。`:'未找到匹配的敏感信息；仍可手动画实心遮盖。'
  }catch(e){if(!disposed&&id===currentId.value)privacyMessage.value=String(e)}finally{privacyBusy.value=false;scheduleRender()}
}
function applyRedactions(){
  if(actionsBlocked.value)return;const candidates=privacyCandidates.value.filter(c=>c.selected);if(!candidates.length)return
  if(annotations.value.length+candidates.length>1000){privacyMessage.value='候选过多，请分批处理。';return}
  checkpoint();annotations.value=[...annotations.value,...candidates.map(c=>({id:crypto.randomUUID(),type:'redact' as const,x:c.x,y:c.y,width:c.width,height:c.height,color:'#000000',lineWidth:1}))];privacyCandidates.value=[];privacyMessage.value=`已遮盖 ${candidates.length} 处，可撤销。`;feedback(privacyMessage.value)
}
function toggleCandidate(id:string){privacyCandidates.value=privacyCandidates.value.map(c=>c.id===id?{...c,selected:!c.selected}:c);scheduleRender()}
function clearCandidates(){privacyCandidates.value=[];privacyMessage.value='';scheduleRender()}
function reportEditorState(){
  if(!desktop||!nativeReady||disposed)return Promise.resolve()
  const state={hasSession:documents.value.length>0,dirty:dirty.value,busy:busy.value||captureBusy.value||updateBusy.value||privacyBusy.value||ioPending.value>0||exitWorking.value||!initialized.value}
  const work=nativeStateQueue.then(()=>invoke<void>('set_editor_state',state));nativeStateQueue=work.catch(()=>{});return work
}
async function cancelExit(){if(exitWorking.value)return;try{await invoke('cancel_exit');exitPending.value=false;exitDialog.value?.close()}catch(e){exitError.value=String(e)}}
async function finishExit(backup:boolean){
  if(exitWorking.value)return;exitWorking.value=true;exitError.value=''
  try{await reportEditorState();if(backup){await createRecoveryPoint(await getSession());dirty.value=false}exitWorking.value=false;await reportEditorState();await invoke('confirm_exit')}
  catch(e){exitError.value=String(e)}finally{exitWorking.value=false}
}
async function receiveExit(){
  if(exitPending.value)return
  if(busy.value||updateBusy.value||privacyBusy.value||ioPending.value||!initialized.value){await invoke('cancel_exit');feedback('请等待当前操作完成后再退出。',true);return}
  exitPending.value=true;exitError.value='';cancelPointer();cancelDelayIfPending()
  try{await reportEditorState();if(!dirty.value){await invoke('confirm_exit');return}}catch(e){exitError.value=String(e)}
  await nextTick();if(!exitDialog.value?.open)exitDialog.value?.showModal()
}
async function idleRelease(requestId:number){
  if(!initialized.value||documents.value.length||ioPending.value||busy.value||captureBusy.value||actionsBlocked.value){await invoke('cancel_idle_release',{requestId});return}
  releasePending.value=true
  try{await invoke('release_empty_editor',{requestId})}catch(e){releasePending.value=false;feedback(String(e),true)}
}
watch([style,annotations,crop,backgroundImage,documents],()=>{dirty.value=documents.value.length>0;sessionRevision.value++;privacyCandidates.value=[];selectedId.value=annotations.value.some(a=>a.id===selectedId.value)?selectedId.value:''},{deep:true,flush:'sync'})
watch([documents,dirty,busy,captureBusy,updateBusy,privacyBusy,ioPending,exitWorking],()=>{void reportEditorState().catch(e=>feedback(String(e),true))},{flush:'sync'})
function pruneStickerImages(){
  const used=new Set<string>();const collect=(list:Annotation[])=>list.forEach(a=>{if(a.type==='sticker'&&a.dataUrl)used.add(a.dataUrl)})
  collect(annotations.value);for(const state of [...undoStack.value,...redoStack.value])collect(state.annotations)
  for(const doc of documents.value){collect(doc.state.annotations);for(const state of [...doc.undo,...doc.redo])collect(state.annotations)}
  for(const key of stickerImages.keys())if(!used.has(key))stickerImages.delete(key)
}
watch([annotations,undoStack,redoStack,documents],pruneStickerImages)
watch([captureDelay,format,outputScale,appearance,customPresets],()=>{
  const preferences=normalizePreferences({captureDelay:captureDelay.value,format:format.value,outputScale:outputScale.value,appearance:appearance.value,presets:customPresets.value})
  try{localStorage.setItem(PREFERENCES_KEY,JSON.stringify(preferences))}catch{feedback('设置已应用，保存偏好失败。',true)}
  applyAppearance(document.documentElement,appearance.value,['moon','plain','sakura'].includes(theme.value))
},{deep:true})
onMounted(async()=>{
  applyAppearance(document.documentElement,appearance.value,['moon','plain','sakura'].includes(theme.value))
  if(!desktop||view.value)return
  try{
    const offExit=await listen('request-exit',()=>void receiveExit())
    const offRelease=await listen<{requestId:number}>('request-idle-release',event=>void idleRelease(event.payload.requestId))
    if(disposed){offExit();offRelease();return}unlisten.push(offExit,offRelease);nativeReady=true;await reportEditorState()
  }catch(e){feedback(String(e),true)}
})

</script>

<template>
  <CaptureOverlay :demo="capturePreview" v-if="view==='capture'||capturePreview" @captured="captured" @cancel="leaveCapture" />
  <PinWindow v-else-if="view==='pin'" />
  <div v-else class="app-shell" @dragover.prevent @drop="onDrop">
    <input ref="importInput" class="sr-only" type="file" accept="image/png,image/jpeg,image/webp" aria-label="导入本地图片" @change="onFile($event)" />
    <input ref="stickerInput" class="sr-only" type="file" accept="image/png" aria-label="导入本地贴纸" @change="importSticker" />
    <input ref="backgroundInput" class="sr-only" type="file" accept="image/png,image/jpeg,image/webp" aria-label="导入背景图片" @change="onFile($event,true)" />
    <header class="app-header" :inert="actionsBlocked || undefined">
      <a class="brand" href="#" @click.prevent="modal=null"><img src="/starshot.svg" alt=""/><div><strong>星镜<span>STARSHOT</span></strong><small>截图 · 标注 · 图片美化</small></div></a>
      <div class="header-actions"><span v-if="!desktop" class="preview-label">浏览器预览</span><button class="button subtle" @click="importInput?.click()"><Icon name="import" :size="16"/>导入图片</button><button class="button primary" :disabled="captureBusy" @click="startCapture"><Icon name="capture" :size="17"/>{{captureBusy?'准备截图…':'开始截图'}}<kbd v-if="desktop">{{shortcutLabel(shortcuts.captureShortcut)}}</kbd></button></div>
    </header>
    <div class="workspace" :inert="actionsBlocked || undefined">
      <nav class="side-rail" aria-label="工作区导航"><div class="rail-top"><button class="rail-button active" title="图片工作台" aria-label="图片工作台" @click="modal=null"><Icon name="image"/></button><button class="rail-button" title="最近图片 · 当前会话" aria-label="最近图片" @click="modal='history'"><Icon name="history"/><i v-if="documents.length"/></button></div><div class="rail-bottom"><span class="local-dot" title="本地处理 · 无上传"/><button class="rail-button" title="外观与使用说明" aria-label="设置与帮助" @click="modal='settings'"><Icon name="settings"/></button></div></nav>
      <main class="main-workspace">
        <div class="document-bar"><div class="document-title"><Icon :name="image?'image':'spark'" :size="16"/><span>{{image?currentName:'你的灵感，值得被好好看见。'}}</span><span v-if="image" class="muted">{{size.width}} × {{size.height}}</span></div><div v-if="image" class="inline-actions"><button class="icon-button" title="撤销 ⌘/Ctrl Z" aria-label="撤销" :disabled="!undoStack.length" @click="undo"><Icon name="undo" :size="17"/></button><button class="icon-button" title="重做 ⇧ ⌘/Ctrl Z" aria-label="重做" :disabled="!redoStack.length" @click="redo"><Icon name="redo" :size="17"/></button><span class="divider"/><button class="button text-button" @click="pinImage"><Icon name="pin" :size="15"/>贴到桌面</button></div></div>
        <div v-if="!image" class="welcome" ref="stage"><div class="welcome-art"><div class="orbit orbit-one"/><div class="orbit orbit-two"/><span class="tiny-star s-one">✦</span><span class="tiny-star s-two">✧</span><div class="art-frame"><Icon name="capture" :size="68"/><div class="frame-cursor"><Icon name="cursor" :size="24"/></div></div></div><div class="eyebrow">A LITTLE SPACE FOR YOUR IDEAS</div><h1>把灵感，定格成星。</h1><p>轻轻一截，再添一点自己的风格。<br/>从屏幕上的一个瞬间，到值得分享的一张好图。</p><div class="welcome-actions"><button class="button primary large" @click="startCapture"><Icon name="capture"/>开始截图</button><button class="button large" @click="importInput?.click()"><Icon name="import"/>导入图片</button></div><button class="demo-link" @click="loadDemo">用一张原创示例，试试星镜 <Icon name="chevron" :size="14"/></button><div class="welcome-features"><div><Icon name="capture"/><strong>聪明地框选</strong><span>窗口吸附 · 像素微调</span></div><div><Icon name="pen"/><strong>清楚地表达</strong><span>箭头文字 · 马赛克</span></div><div><Icon name="spark"/><strong>好看地分享</strong><span>背景边框 · 圆角阴影</span></div></div></div>
        <template v-else><div class="editor-area"><div class="tool-rail" role="toolbar" aria-label="标注工具"><button v-for="t in tools" :key="t.id" class="tool-button" :class="{selected:tool===t.id}" :title="`${t.name} (${t.key})`" :aria-label="t.name" :aria-pressed="tool===t.id" @click="selectTool(t.id)"><Icon :name="t.id" :size="20"/></button><span class="tool-divider"/><input v-model="color" type="color" aria-label="标注颜色" title="标注颜色" class="tool-color"/></div><div ref="stage" class="canvas-stage"><div class="canvas-position" :style="{width:`${displayWidth}px`,height:`${displayHeight}px`}"><canvas ref="canvas" aria-label="图片编辑画布" :style="{width:`${displayWidth}px`,height:`${displayHeight}px`,cursor:tool==='cursor'?'default':'crosshair'}" @pointerdown="pointerDown" @pointermove="pointerMove" @pointerup="pointerUp" @pointercancel="cancelPointer" @lostpointercapture="cancelPointer"/></div></div></div><div class="canvas-footer"><span>{{annotationHint}}</span><div class="zoom-control"><button aria-label="缩小预览" :disabled="zoom<=.3" @click="zoom=Math.max(.25,zoom-.25)">−</button><button @click="zoom=1" title="适应画布">{{Math.round(fit*zoom*100)}}%</button><button aria-label="放大预览" :disabled="zoom>=3" @click="zoom=Math.min(3,zoom+.25)">＋</button></div></div></template>
      </main>
      <aside class="inspector" :class="{inactive:!image}"><div class="inspector-heading"><h2>让截图，再好看一点</h2><p>小小的调整，刚刚好的表达。</p></div><div class="panel-tabs" role="tablist" aria-label="编辑面板"><button :class="{active:panel==='beautify'}" role="tab" :aria-selected="panel==='beautify'" @click="panel='beautify'">图片美化</button><button :class="{active:panel==='annotate'}" role="tab" :aria-selected="panel==='annotate'" @click="panel='annotate'">标注工具</button></div>
        <div class="inspector-scroll"><fieldset :disabled="!image"><template v-if="panel==='beautify'"><section class="control-section"><div class="section-label"><div class="control-heading"><h3>画布背景</h3><button type="button" class="button text-button" :disabled="!hasCustomStyle" @click="resetStyle">恢复默认美化</button></div><Icon name="spark" :size="14"/></div><div class="segmented"><button v-for="bg in [{id:'gradient',name:'渐变'},{id:'solid',name:'纯色'},{id:'transparent',name:'透明'},{id:'image',name:'图片'}]" :key="bg.id" :class="{active:style.background===bg.id}" @click="bg.id==='image'&&!backgroundImage?backgroundInput?.click():setStyle('background',bg.id as EditorStyle['background'])">{{bg.name}}</button></div><div v-if="style.background==='gradient'" class="gradient-grid"><button v-for="p in presets" :key="p.name" class="gradient-swatch" :style="{background:`linear-gradient(${p.angle}deg,${p.from},${p.to})`}" :title="p.name" :aria-label="p.name" :aria-pressed="style.gradientFrom===p.from&&style.gradientTo===p.to" @click="usePreset(p)"><Icon v-if="style.gradientFrom===p.from&&style.gradientTo===p.to" name="check" :size="18"/></button></div><div v-if="style.background==='gradient'" class="color-pair"><label><input type="color" aria-label="渐变起始颜色" :value="style.gradientFrom" @input="setStyle('gradientFrom',($event.target as HTMLInputElement).value)"/><span>起始</span></label><label><input type="color" aria-label="渐变结束颜色" :value="style.gradientTo" @input="setStyle('gradientTo',($event.target as HTMLInputElement).value)"/><span>结束</span></label><input class="angle-input" type="number" min="0" max="360" aria-label="渐变角度" :value="style.gradientAngle" @change="setStyle('gradientAngle',Math.max(0,Math.min(360,Number(($event.target as HTMLInputElement).value))))"/><span>°</span></div><label v-if="style.background==='solid'" class="color-line"><span>背景颜色</span><input type="color" aria-label="背景颜色" :value="style.backgroundColor" @input="setStyle('backgroundColor',($event.target as HTMLInputElement).value)"/></label><p v-if="style.background==='transparent'" class="field-note">PNG 保留透明，JPEG 使用白底。</p><button v-if="style.background==='image'" class="button full" @click="backgroundInput?.click()"><Icon name="image" :size="15"/>更换本地背景</button></section>
        <section class="control-section"><h3>我的美化预设</h3><div class="preset-save"><input v-model="presetName" aria-label="美化预设名称" maxlength="30"/><button class="button" @click="savePreset">保存</button></div><div v-for="(saved,index) in customPresets" :key="saved.name" class="preset-row"><button class="button" @click="applySavedPreset(index)">{{saved.name}}</button><button class="icon-button" :aria-label="`删除预设 ${saved.name}`" @click="customPresets=customPresets.filter((_,i)=>i!==index)">×</button></div><p class="field-note">仅保存美化参数，不保存原图或本地背景图。</p></section><section class="control-section"><h3>构图与留白</h3><label class="select-line"><span>画布比例</span><select aria-label="画布比例" :value="style.aspect" @change="setStyle('aspect',($event.target as HTMLSelectElement).value as EditorStyle['aspect'])"><option value="auto">自适应</option><option value="1:1">1:1 · 方形</option><option value="4:3">4:3 · 横向</option><option value="16:9">16:9 · 宽屏</option><option value="9:16">9:16 · 竖屏</option></select></label><label class="range-field"><span>四周留白 <output>{{style.padding}} px</output></span><input aria-label="四周留白" type="range" min="0" max="200" step="4" :value="style.padding" @input="setStyle('padding',Number(($event.target as HTMLInputElement).value))"/></label><label class="range-field"><span>图片圆角 <output>{{style.radius}} px</output></span><input aria-label="图片圆角" type="range" min="0" max="100" :value="style.radius" @input="setStyle('radius',Number(($event.target as HTMLInputElement).value))"/></label></section>
        <section class="control-section"><h3>边框与层次</h3><label class="range-field"><span>边框宽度 <output>{{style.borderWidth}} px</output></span><input aria-label="边框宽度" type="range" min="0" max="24" :value="style.borderWidth" @input="setStyle('borderWidth',Number(($event.target as HTMLInputElement).value))"/></label><label class="color-line"><span>边框颜色</span><input type="color" aria-label="边框颜色" :value="style.borderColor" @input="setStyle('borderColor',($event.target as HTMLInputElement).value)"/></label><label class="range-field"><span>柔和阴影 <output>{{style.shadow}} px</output></span><input aria-label="柔和阴影" type="range" min="0" max="80" :value="style.shadow" @input="setStyle('shadow',Number(($event.target as HTMLInputElement).value))"/></label></section></template>
        <template v-else><PrivacyPanel :candidates="privacyCandidates" :busy="privacyBusy" :desktop="desktop" :message="privacyMessage" @scan="scanPrivacy" @apply="applyRedactions" @clear="clearCandidates" @toggle="toggleCandidate"/><section class="control-section"><h3>对象与贴纸</h3><button class="button full" @click="stickerInput?.click()">导入透明 PNG 贴纸</button><select class="object-select" aria-label="选择标注对象" v-model="selectedId" @change="tool='cursor';scheduleRender()"><option value="">选择已有标注或贴纸</option><option v-for="(a,i) in annotations" :key="a.id" :value="a.id">{{i+1}} · {{a.type==='sticker'?'贴纸':a.type==='redact'?'实心遮盖':a.type==='text'?'文字':a.type}}</option></select><div v-if="selectedAnnotation" class="selected-controls"><label v-if="selectedAnnotation.type!=='redact'&&selectedAnnotation.type!=='sticker'" class="color-line">对象颜色<input type="color" aria-label="选中对象颜色" :value="selectedAnnotation.color" @input="editSelected({color:($event.target as HTMLInputElement).value})"/></label><textarea v-if="selectedAnnotation.type==='text'" aria-label="编辑选中文字" :value="selectedAnnotation.text" maxlength="1000" @change="editSelected({text:($event.target as HTMLTextAreaElement).value})"/><template v-if="selectedAnnotation.type==='sticker'"><label class="range-field"><span>贴纸旋转 <output>{{selectedAnnotation.rotation||0}}°</output></span><input type="range" min="0" max="360" aria-label="贴纸旋转" :value="selectedAnnotation.rotation||0" @input="editSelected({rotation:Number(($event.target as HTMLInputElement).value)})"/></label><button class="button" @click="scaleSelected(90)">缩小贴纸</button><button class="button" @click="scaleSelected(110)">放大贴纸</button></template><button class="button full" @click="deleteSelected">删除选中对象</button></div></section><section class="control-section"><h3>{{tools.find(t=>t.id===tool)?.name || '标注'}}</h3><p class="field-note">{{annotationHint}}</p><div class="annotation-grid"><button v-for="t in tools.filter(t=>t.id!=='cursor')" :key="t.id" :class="{active:tool===t.id}" @click="selectTool(t.id)"><Icon :name="t.id" :size="18"/><span>{{t.name}}</span></button></div></section><section class="control-section"><label class="color-line"><span>标注颜色</span><input v-model="color" type="color" aria-label="工具颜色"/></label><label class="range-field"><span>{{tool==='mosaic'?'马赛克强度':'线条粗细'}} <output>{{lineWidth}} px</output></span><input v-model.number="lineWidth" type="range" min="2" max="30" aria-label="线条粗细"/></label><template v-if="tool==='text'||tool==='number'"><label class="range-field"><span>文字大小 <output>{{fontSize}} px</output></span><input v-model.number="fontSize" type="range" min="12" max="96" aria-label="文字大小"/></label><textarea v-if="tool==='text'" v-model="text" aria-label="标注文字" placeholder="输入要添加的文字" maxlength="1000" rows="3"/><button v-if="tool==='text'" class="button full" @click="addWatermark">作为右下角水印</button></template><button v-if="crop" class="button full" @click="resetCrop"><Icon name="crop" :size="16"/>恢复原图范围</button><button class="button full subtle" :disabled="!annotations.length" @click="removeAnnotations"><Icon name="trash" :size="15"/>清空标注（可撤销）</button><p class="field-note">{{annotations.length}} 处标注 · 原图始终保留</p></section></template>
        </fieldset></div>
        <div class="export-panel"><label class="export-name"><span>导出文件名</span><input v-model="exportName" aria-label="导出文件名" :disabled="!image" maxlength="100"/></label><div class="export-options"><select v-model="format" aria-label="导出格式"><option value="png">PNG</option><option value="jpeg">JPEG</option></select><select v-model="outputScale" aria-label="导出倍率"><option :value="1">1× 原尺寸</option><option :value="2">2× 高清</option></select><span>{{image?outputSize:'—'}}</span></div><div class="export-buttons"><button class="button copy-button" :disabled="!image||busy||outputSize==='尺寸超限'" title="复制图片" aria-label="复制图片" @click="copyImage"><Icon name="copy" :size="18"/></button><button class="button primary export-button" :disabled="!image||busy||outputSize==='尺寸超限'" @click="exportImage"><Icon name="save" :size="17"/>{{busy?'处理中…':'导出图片'}}</button></div></div>
      </aside>
    </div>
    <footer class="status-bar" :class="{error:isError||renderError}" role="status"><span class="status-message"><span class="status-led"/>{{renderError||status}}</span><button v-if="captureBusy&&captureDelay" class="status-cancel" @click="cancelDelay">取消截图</button><button v-if="recoveryAvailable&&!updateBusy" class="status-cancel" @click="modal='settings'">可恢复升级前会话</button><button v-if="savedPath&&desktop&&!actionsBlocked" class="status-cancel" @click="invoke('reveal_saved_image').then(()=>feedback('已请求在文件管理器中显示。')).catch(e=>feedback(String(e),true))">显示已保存文件</button><span class="status-brand">LOCAL FIRST<span>✦</span>星镜 {{appVersion}}</span></footer>
    <div v-show="modal" class="modal-backdrop" @click.self="closeModal"><section class="dialog" role="dialog" aria-modal="true" :aria-label="modal==='history'?'最近图片':'设置与帮助'"><header><div><h2>{{modal==='history'?'最近图片':'星镜 · 设置与帮助'}}</h2><p>{{modal==='history'?'仅保留当前会话，关闭软件后清空。':'本地优先，让每一次截图都安心。'}}</p></div><button class="icon-button" aria-label="关闭弹窗" :disabled="updateBusy" @click="closeModal"><Icon name="close"/></button></header><template v-if="modal==='history'"><div v-if="!documents.length" class="empty-history"><Icon name="image" :size="32"/><p>还没有图片，先截取一张吧。</p></div><div class="history-grid"><button v-for="doc in documents" :key="doc.id" @click="openDocument(doc)"><img :src="doc.dataUrl" alt=""/><span>{{doc.name}}</span><small>{{doc.width}} × {{doc.height}}</small></button></div></template><div v-show="modal==='settings'"><fieldset :disabled="updateBusy"><section class="settings-section"><h3>界面外观</h3><div class="theme-options"><button v-for="t in themes" :key="t.id" :class="{active:theme===t.id}" @click="theme=t.id">{{t.label}}</button></div></section><section class="settings-section"><h3>自定义界面配色</h3><label class="field-note"><input v-model="appearance.enabled" type="checkbox"/>启用自己的配色</label><div class="color-pair"><label>起始<input v-model="appearance.from" type="color" aria-label="界面起始颜色"/></label><label>结束<input v-model="appearance.to" type="color" aria-label="界面结束颜色"/></label><input v-model.number="appearance.angle" type="number" min="0" max="360" aria-label="界面渐变方向"/>°</div><p class="field-note">界面配色与导出图片的背景分别设置。</p></section><DesktopSettings @shortcuts="shortcuts=$event"/><section class="settings-section"><label class="select-line"><span>延时截图</span><select v-model.number="captureDelay" aria-label="延时截图"><option :value="0">立即</option><option :value="3">3 秒</option><option :value="5">5 秒</option><option :value="10">10 秒</option></select></label><p class="field-note">延时只作用于「开始截图」按钮，全局快捷键立即截图。</p></section><section class="settings-section shortcut-list"><h3>快捷操作</h3><p class="field-note">关闭主窗口会收起到托盘。退出前请先备份会话，再从托盘选择「退出星镜」；Mac 的 ⌘Q 也会检查未备份进度，Dock 菜单退出暂不提供这项保护。</p><div><span>开始截图</span><kbd>{{shortcutLabel(shortcuts.captureShortcut)}}</kbd></div><div><span>贴出最近截图原图</span><kbd>{{shortcutLabel(shortcuts.pinShortcut)}}</kbd></div><div><span>导出图片 / 撤销</span><kbd>⌘ / Ctrl + S / Z</kbd></div><div><span>复制截图 / 取消截图</span><kbd>Enter / Esc</kbd></div></section><section class="settings-section"><h3>关于这个版本</h3><p class="field-note">截图按需触发，不监听剪贴板，不上传图片。当前会话最多保留 6 张原图与各自编辑，内存预算 4800 万像素；普通编辑只留在内存，导出、手动备份或安装更新时才写入图片文件。</p><p class="field-note">区域预判结合窗口边界和图片边缘；Mac 已有辅助功能授权时可查询控件边界。选区工具条支持手动逐屏长截图；贴图可双击关闭、右键操作。敏感信息识别仅在点击时使用本地 OCR。macOS 首次截图需要系统屏幕录制权限。</p><button class="button full" @click="modal=null;capturePreview=true">体验区域选择（原创示例）</button></section></fieldset><ShareSettings :disabled="updateBusy||exitPending||privacyBusy"/><UpdateSettings :disabled="busy||captureBusy||ioPending>0||privacyBusy||exitPending||releasePending" :get-session="getSession" :restore-session="restoreSession" @busy="setUpdateBusy" @status="receiveUpdateStatus" @recovery-available="recoveryAvailable=$event" @backed-up="dirty=false" /></div></section></div>
    <dialog ref="exitDialog" class="dialog exit-dialog" @cancel.prevent="cancelExit"><h2>退出前保留编辑进度？</h2><p class="field-note">当前会话尚有未备份的修改。图片文件导出不包含可继续编辑的状态。</p><p v-if="exitError" class="field-note" role="alert">{{exitError}}</p><div class="exit-actions"><button class="button" :disabled="exitWorking" @click="cancelExit">取消</button><button class="button" :disabled="exitWorking" @click="finishExit(false)">直接退出</button><button class="button primary" :disabled="exitWorking" @click="finishExit(true)">{{exitWorking?'正在保存…':'备份并退出'}}</button></div></dialog>
  </div>
</template>
