<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

const desktop = '__TAURI_INTERNALS__' in window
const image = ref<{ dataUrl: string; width: number; height: number }>()
const zoom = ref(1)
const opacity = ref(1)
const rotation = ref(0)
const flipped = ref(false)
const toolbarVisible = ref(true)
const pinHalo = ref(true)
const focused = ref(!desktop && document.hasFocus())
const gutter = ref(12)
const error = ref('')
const status = ref('')
const busy = ref<'' | 'copy' | 'save' | 'edit'>('')
const modKey = navigator.platform.toLowerCase().includes('mac') ? '⌘' : 'Ctrl'
const opacityKey = modKey === '⌘' ? '⌘ 或 Ctrl' : 'Ctrl'
const imageHint = [
  desktop ? '双击关闭这张贴图；按住图片可拖动。' : '双击退出贴图演示。',
  `滚轮缩放；按住 ${opacityKey} 滚动可调透明度。`,
  '右键打开操作菜单；空格显示或隐藏工具栏。',
  desktop ? '光圈可在右键菜单或「设置与帮助 → 贴图焦点光圈」中关闭。' : '光圈可在右键菜单中关闭。',
].join('\n')
const passthroughHint = '开启后，鼠标操作会交给贴图下方的窗口。\n从托盘选择「恢复所有贴图鼠标操作」即可恢复。'
const imageArea = ref<HTMLElement>()
const menuElement = ref<HTMLElement>()
const contextMenu = ref<{ x: number; y: number }>()
let closing = false
let lastPress: { time: number; x: number; y: number } | undefined
const viewport = ref({ width: window.innerWidth, height: window.innerHeight })
const unlisteners: UnlistenFn[] = []
let focusRevision = 0
let haloRevision = 0
let mounted = true
let scaleQueue = Promise.resolve()
let confirmedZoom = 1
// Fit the actual rotated image, so the halo never outlines transparent letterboxing.
const displaySize = computed(() => {
  const width = image.value?.width ?? 1, height = image.value?.height ?? 1
  const rotated = rotation.value % 180 !== 0
  const scale = Math.max(0, Math.min(
    (viewport.value.width - gutter.value * 2) / (rotated ? height : width),
    (viewport.value.height - gutter.value * 2) / (rotated ? width : height),
  )) * (desktop ? 1 : zoom.value)
  return { width: width * scale, height: height * scale, rotated }
})
const frameStyle = computed(() => ({
  width: `${displaySize.value.rotated ? displaySize.value.height : displaySize.value.width}px`,
  height: `${displaySize.value.rotated ? displaySize.value.width : displaySize.value.height}px`,
}))
const imageStyle = computed(() => ({
  width: `${displaySize.value.width}px`, height: `${displaySize.value.height}px`, opacity: opacity.value,
  transform: `rotate(${rotation.value}deg) scaleX(${flipped.value ? -1 : 1})`,
}))
function track(off: UnlistenFn) { if (mounted) unlisteners.push(off); else off() }
async function toggleHalo() {
  const next = !pinHalo.value, revision = haloRevision
  if (await action('halo', Number(next)) && revision === haloRevision) pinHalo.value = next
}
async function action(actionName: string, value?: number) {
  if (!desktop) return true
  try { await invoke('pin_action', { action: actionName, value }); return true }
  catch (cause) { error.value = String(cause); return false }
}
function drag(event: PointerEvent) {
  if (busy.value) return
  if (contextMenu.value) { dismissMenu(); return }
  if (event.ctrlKey) return // macOS Control + 单击交给右键菜单。
  if (event.button !== 0 || !desktop) return
  // 原生起拖可能吞掉 click / dblclick；同一屏幕位置连续按下也能关闭。
  // 用屏幕坐标区分「拖走后又按下」，不把移动后的相同窗内坐标算成双击。
  if (lastPress && event.timeStamp - lastPress.time <= 500 && Math.hypot(event.screenX - lastPress.x, event.screenY - lastPress.y) <= 4) {
    lastPress = undefined
    event.preventDefault()
    void close()
    return
  }
  lastPress = { time: event.timeStamp, x: event.screenX, y: event.screenY }
  // 原生拖动必须直接由按下事件发起，等待移动或长按会错失 macOS 起拖时机。
  void action('drag')
}
function moved(event: PointerEvent) {
  if (lastPress && event.buttons === 1 && Math.hypot(event.screenX - lastPress.x, event.screenY - lastPress.y) > 4) lastPress = undefined
}
function doubleClick(event: MouseEvent) {
  if (event.button === 0 && !event.ctrlKey && !contextMenu.value) void close()
}
function setZoom(value: number) {
  if (busy.value) return
  const next = Math.max(.1, Math.min(4, Math.round(value * 100) / 100))
  zoom.value = next
  if (desktop) scaleQueue = scaleQueue.then(async () => {
    if (await action('scale', next)) confirmedZoom = next
    else if (zoom.value === next) zoom.value = confirmedZoom
  })
}
function wheel(event: WheelEvent) {
  if (busy.value || !event.deltaY) return
  event.preventDefault()
  if (event.ctrlKey || event.metaKey) setOpacity(opacity.value + (event.deltaY < 0 ? .05 : -.05))
  else setZoom(zoom.value * (event.deltaY < 0 ? 1.1 : 1 / 1.1))
}
function setOpacity(value: number) {
  if (busy.value) return
  opacity.value = Math.max(.15, Math.min(1, Math.round(value * 100) / 100))
  void action('opacity', opacity.value)
}
function reset() { if (busy.value) return; rotation.value = 0; flipped.value = false; setZoom(1); setOpacity(1) }
function resized() { viewport.value = { width: window.innerWidth, height: window.innerHeight }; dismissMenu(false) }
async function close() {
  if (closing || busy.value) return
  closing = true
  dismissMenu(false)
  if (desktop) { if (!await action('close')) closing = false }
  else window.location.assign('/')
}
async function exportPin(actionName: 'copy' | 'save' | 'edit') {
  if (!desktop || !image.value || busy.value || closing) return
  busy.value = actionName; error.value = ''; status.value = ''
  dismissMenu(false)
  try {
    const result = await invoke<{ completed: boolean; path?: string; warning?: string }>('pin_export', { action: actionName, rotation: rotation.value, flipped: flipped.value })
    if (!mounted) return
    status.value = !result.completed ? '已取消保存，贴图保留。' : actionName === 'copy' ? '图片已复制。' : actionName === 'save' ? `已保存到 ${result.path}` : '已发送到编辑器，贴图仍保留。'
    if (result.warning) status.value += ` ${result.warning}`
  } catch (cause) { if (mounted) error.value = String(cause) }
  finally { busy.value = '' }
}
const menuItems = computed(() => [
  ...(desktop ? [
    { label: '复制图片', hint: `${modKey}+C`, run: () => exportPin('copy') },
    { label: '另存为…', hint: `${modKey}+S`, run: () => exportPin('save') },
    { label: '送到编辑器', hint: `${modKey}+E`, run: () => exportPin('edit') },
  ] : []),
  { label: '放大贴图', hint: '+', run: () => setZoom(zoom.value * 1.1) },
  { label: '缩小贴图', hint: '−', run: () => setZoom(zoom.value / 1.1) },
  { label: '恢复大小、方向与透明度', hint: '0', run: reset },
  { label: '向右旋转 90°', hint: 'R', run: () => { rotation.value = (rotation.value + 90) % 360 } },
  { label: '水平翻转', hint: 'F', run: () => { flipped.value = !flipped.value } },
  { label: '降低不透明度', hint: `${Math.round(opacity.value * 100)}%`, run: () => setOpacity(opacity.value - .1) },
  { label: '提高不透明度', hint: '', run: () => setOpacity(opacity.value + .1) },
  ...(desktop ? [{ label: '开启鼠标穿透', hint: '托盘可恢复', tooltip: passthroughHint, run: () => { void action('ignore-cursor', 1) } }] : []),
  { label: pinHalo.value ? '关闭贴图光圈' : '开启贴图光圈', hint: desktop ? '所有贴图' : '', tooltip: desktop ? '对所有贴图生效，选择会保存在本机。\n也可在「设置与帮助 → 贴图焦点光圈」中修改。' : '仅切换演示中的光圈，不改变图片大小。', run: toggleHalo },
  { label: toolbarVisible.value ? '隐藏工具栏' : '显示工具栏', hint: '空格', run: () => { toolbarVisible.value = !toolbarVisible.value } },
  { label: '关闭贴图', hint: '双击 / Esc', run: close },
])
function dismissMenu(focusImage = true) {
  lastPress = undefined
  if (!contextMenu.value) return
  contextMenu.value = undefined
  if (focusImage) imageArea.value?.focus({ preventScroll: true })
}
async function openMenu(event?: MouseEvent) {
  if (closing || busy.value) return
  lastPress = undefined
  contextMenu.value = { x: event?.clientX ?? viewport.value.width / 2, y: event?.clientY ?? viewport.value.height / 2 }
  await nextTick()
  if (!contextMenu.value || !menuElement.value) return
  const bounds = menuElement.value.getBoundingClientRect()
  contextMenu.value = {
    x: Math.max(8, Math.min(contextMenu.value.x, viewport.value.width - bounds.width - 8)),
    y: Math.max(8, Math.min(contextMenu.value.y, viewport.value.height - bounds.height - 8)),
  }
  menuElement.value.querySelector<HTMLButtonElement>('button')?.focus({ preventScroll: true })
}
function runMenuItem(run: () => unknown) { dismissMenu(); run() }
function blurred() { dismissMenu(false); if (!desktop) focused.value = false }
function gainedFocus() { if (!desktop) focused.value = true }
function keydown(event: KeyboardEvent) {
  if (busy.value) { event.preventDefault(); return }
  if (desktop && (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey) {
    const command = ({ c: 'copy', s: 'save', e: 'edit' } as const)[event.key.toLowerCase() as 'c' | 's' | 'e']
    if (command) { event.preventDefault(); void exportPin(command); return }
  }
  if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) { event.preventDefault(); void openMenu(); return }
  if (contextMenu.value) {
    if (event.key === 'Escape' || event.key === 'Tab') { event.preventDefault(); dismissMenu(); return }
    if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      event.preventDefault()
      const items = [...(menuElement.value?.querySelectorAll<HTMLButtonElement>('button') ?? [])]
      if (!items.length) return
      const active = items.indexOf(document.activeElement as HTMLButtonElement)
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (active + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length
      items[next]?.focus()
    }
    return
  }
  if (event.key === 'Escape') { event.preventDefault(); void close(); return }
  if (event.target instanceof HTMLInputElement) return
  switch (event.key) {
    case '+': case '=': event.preventDefault(); setZoom(zoom.value * 1.1); break
    case '-': event.preventDefault(); setZoom(zoom.value / 1.1); break
    case '0': event.preventDefault(); reset(); break
    case 'r': case 'R': rotation.value = (rotation.value + 90) % 360; break
    case 'f': case 'F': flipped.value = !flipped.value; break
    case ' ': event.preventDefault(); toolbarVisible.value = !toolbarVisible.value; break
  }
}
onMounted(async () => {
  document.documentElement.classList.add('starshot-pin-document')
  window.addEventListener('keydown', keydown)
  window.addEventListener('resize', resized)
  window.addEventListener('blur', blurred)
  window.addEventListener('focus', gainedFocus)
  try {
    if (desktop) {
      await Promise.all([
        getCurrentWindow().onFocusChanged(event => { focusRevision++; focused.value = event.payload; if (!event.payload) dismissMenu(false) }).then(track),
        listen<boolean>('pin-halo', event => { haloRevision++; pinHalo.value = event.payload }).then(track),
        getCurrentWindow().listen<number>('pin-opacity', event => { opacity.value = event.payload }).then(track),
      ])
      if (!mounted) return
      const revision = haloRevision, focusAtRequest = focusRevision
      const loaded = await invoke<{ dataUrl: string; width: number; height: number; pinHalo: boolean; focused: boolean; gutter: number }>('get_pin_image')
      if (!mounted) return
      image.value = loaded
      gutter.value = loaded.gutter ?? 12
      if (revision === haloRevision) pinHalo.value = loaded.pinHalo ?? true
      if (focusAtRequest === focusRevision) focused.value = loaded.focused ?? false
    } else image.value = { dataUrl: '/demo-workspace.svg', width: 1200, height: 750 }
  } catch (cause) { if (mounted) error.value = String(cause) }
})
onBeforeUnmount(() => { mounted = false; unlisteners.forEach(off => off()); window.removeEventListener('keydown', keydown); window.removeEventListener('resize', resized); window.removeEventListener('blur', blurred); window.removeEventListener('focus', gainedFocus); document.documentElement.classList.remove('starshot-pin-document') })
</script>

<template>
  <main class="pin-window" :class="{ 'pin-focused': focused, 'pin-halo': pinHalo }" :style="{ '--pin-gutter': `${gutter}px` }" :aria-label="`星镜置顶贴图（${focused ? '活跃' : '未活跃'}）`" @wheel="wheel" @contextmenu.prevent="openMenu">
    <div ref="imageArea" class="pin-image-area" tabindex="0" aria-label="贴图：拖动移动，双击关闭，右键打开操作菜单" :title="imageHint" :aria-description="imageHint" @pointerdown="drag" @pointermove="moved" @dblclick.prevent="doubleClick">
      <div v-if="image" class="pin-frame" :style="frameStyle"><img :src="image.dataUrl" :style="imageStyle" draggable="false" alt="置顶参考图片" /></div>
      <span v-else-if="!error" class="pin-loading">正在加载贴图…</span>
    </div>
    <div v-show="toolbarVisible" class="pin-toolbar" aria-label="贴图工具" @pointerdown.stop="dismissMenu(false)">
      <span class="pin-mark" :title="imageHint">✦</span>
      <button type="button" :disabled="!!busy" aria-label="缩小贴图" title="缩小贴图（−）；也可向下滚动滚轮。" @click="setZoom(zoom / 1.1)">−</button>
      <button type="button" :disabled="!!busy" class="pin-scale" title="恢复 100% 缩放、原方向与不透明显示（0）" @click="reset">{{ Math.round(zoom * 100) }}%</button>
      <button type="button" :disabled="!!busy" aria-label="放大贴图" title="放大贴图（+）；也可向上滚动滚轮。" @click="setZoom(zoom * 1.1)">+</button>
      <i />
      <button type="button" :disabled="!!busy" aria-label="向右旋转图片" title="旋转 90°（R）" @click="rotation = (rotation + 90) % 360">↻</button>
      <button type="button" :disabled="!!busy" aria-label="水平翻转图片" title="水平翻转（F）" @click="flipped = !flipped">↔</button>
      <label class="pin-opacity" :title="`调整贴图不透明度；也可按住 ${opacityKey} 滚动滚轮。\n仅影响桌面预览，复制和保存仍保留原图透明度。`"><span>◐</span><input :disabled="!!busy" aria-label="贴图不透明度" type="range" min="0.15" max="1" step="0.05" :value="opacity" @input="setOpacity(Number(($event.target as HTMLInputElement).value))" /></label>
      <i />
      <button v-if="desktop" type="button" :disabled="!!busy" aria-label="开启鼠标穿透" :title="passthroughHint" @click="action('ignore-cursor',1)">⇥</button>
      <button type="button" :disabled="!!busy" aria-label="隐藏贴图工具栏" title="隐藏工具栏（空格）；右键菜单可恢复" @click="toolbarVisible = false">⌄</button>
      <button type="button" :disabled="!!busy" class="pin-close" aria-label="关闭贴图" title="关闭这张贴图（Esc）；也可双击图片。" @click="close">×</button>
    </div>
    <button v-if="!toolbarVisible" :disabled="!!busy" class="pin-reveal" aria-label="显示贴图工具栏" title="显示工具栏（空格）" @pointerdown.stop="dismissMenu(false)" @click="toolbarVisible = true">✦</button>
    <div v-if="contextMenu" ref="menuElement" class="pin-context-menu" role="menu" aria-label="贴图操作" :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }" @pointerdown.stop @wheel.stop>
      <p v-if="desktop" class="pin-export-note">按当前方向输出原始像素；缩放与透明度只影响桌面预览。</p>
      <button v-for="item in menuItems" :key="item.label" type="button" role="menuitem" tabindex="-1" :title="item.tooltip || `${item.label}${item.hint ? `（${item.hint}）` : ''}`" @click="runMenuItem(item.run)"><span>{{ item.label }}</span><small>{{ item.hint }}</small></button>
    </div>
    <div v-if="error" role="alert" class="pin-error">{{ error }}<button type="button" @click="error = ''">关闭提示</button></div>
    <div v-else-if="busy || status" role="status" class="pin-status">{{ busy ? (busy === 'save' ? '正在保存…' : busy === 'copy' ? '正在复制…' : '正在发送到编辑器…') : status }}<button v-if="!busy" type="button" @click="status = ''">关闭提示</button></div>
    <span v-if="!desktop" class="pin-demo-note">浏览器演示 · 桌面版支持置顶与拖动</span>
  </main>
</template>

<style>
html.starshot-pin-document,html.starshot-pin-document body,html.starshot-pin-document #app{background:transparent!important;margin:0;width:100%;height:100%;overflow:hidden}
</style>
<style scoped>
.pin-frame{position:relative;display:flex;align-items:center;justify-content:center;flex-shrink:0}
.pin-frame::after{content:"";position:absolute;inset:-1px;box-sizing:border-box;pointer-events:none}
.pin-halo .pin-frame::after{border:1px solid #7c8797;box-shadow:0 1px 6px #10182060}
.pin-halo.pin-focused .pin-frame::after{border-color:#65b2ff;box-shadow:0 0 7px 1px #3899ff70,0 0 2px #60afffcc}
.pin-export-note{margin:3px 5px 5px;color:#a4b1c8;font-size:10px;line-height:1.6}.pin-toolbar button:disabled,.pin-reveal:disabled{opacity:.4;cursor:wait}.pin-status{position:absolute;bottom:9px;left:9px;right:9px;max-height:calc(100vh - 18px);box-sizing:border-box;overflow:auto;padding:10px;border:1px solid #cfe5a64d;border-radius:8px;background:#19291cf5;color:#e5f5d1;font-size:11px;line-height:1.6;overflow-wrap:anywhere}.pin-status button{display:block;margin-top:5px;background:transparent;border:0;color:inherit;text-decoration:underline;cursor:pointer}
.pin-context-menu{position:absolute;z-index:4;box-sizing:border-box;width:248px;max-width:calc(100vw - 16px);max-height:calc(100vh - 16px);overflow:auto;padding:5px;border:1px solid #ffffff30;border-radius:10px;background:#141c2bf7;box-shadow:0 8px 28px #0006}.pin-context-menu button{display:flex;align-items:center;justify-content:space-between;gap:12px;width:100%;padding:9px 10px;border:0;border-radius:5px;background:transparent;color:#e5ecfa;font:inherit;text-align:left;cursor:pointer}.pin-context-menu button:hover,.pin-context-menu button:focus{outline:none;background:#ffffff17}.pin-context-menu button:last-child{color:#ffbbb3;border-top:1px solid #ffffff1a;border-radius:0 0 5px 5px}.pin-context-menu small{font-size:10px;color:#a4b1c8;white-space:nowrap}.pin-image-area:focus-visible{outline:2px solid #dff5a4;outline-offset:-3px}
.pin-window{position:fixed;inset:0;color:#edf2fc;font:12px Inter,"PingFang SC","Microsoft YaHei",sans-serif;user-select:none;overflow:hidden;background:transparent}.pin-image-area{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;cursor:grab;overflow:hidden}.pin-image-area:active{cursor:grabbing}.pin-image-area img{display:block;max-width:none;max-height:none;flex-shrink:0;object-fit:contain;pointer-events:none;transform-origin:center}.pin-toolbar{position:absolute;z-index:2;display:flex;align-items:center;justify-content:center;gap:2px;left:50%;top:9px;transform:translateX(-50%);padding:5px;border:1px solid #ffffff24;border-radius:9px;background:#121827ed;box-shadow:0 5px 24px #0004;white-space:nowrap;max-width:calc(100vw - 16px)}.pin-toolbar button{display:grid;place-items:center;flex-shrink:0;min-width:25px;height:28px;padding:0 5px;color:#dbe5f5;background:none;border:0;border-radius:5px;font-family:inherit;font-size:16px;cursor:pointer}.pin-toolbar button:hover{background:#ffffff18}.pin-toolbar button:focus-visible,.pin-reveal:focus-visible{outline:2px solid #dff5a4;outline-offset:2px}.pin-mark{display:block;padding:0 6px;color:#dff5a4;font-size:16px}.pin-toolbar .pin-scale{font-size:10px;font-variant-numeric:tabular-nums;min-width:40px}.pin-toolbar>i{width:1px;height:17px;background:#ffffff17;margin:0 4px}.pin-opacity{display:flex;gap:4px;align-items:center;padding:0 3px;color:#b4c0d6}.pin-opacity input{width:55px;accent-color:#dff5a4;cursor:pointer}.pin-close:hover{color:#ffbbb3!important}.pin-reveal{position:absolute;right:7px;top:7px;border:1px solid #ffffff35;border-radius:6px;background:#151c2cd9;color:#dff5a4;width:26px;height:26px;cursor:pointer;opacity:.25}.pin-reveal:hover,.pin-reveal:focus-visible{opacity:1}.pin-loading{font-size:12px;color:#aab5cb;background:#171e2c;padding:12px 18px;border-radius:9px}.pin-error{position:absolute;bottom:9px;left:9px;right:9px;padding:10px;border:1px solid #eaa9a94d;border-radius:8px;background:#341e28f5;color:#ffd1cc;font-size:11px;line-height:1.6;overflow-wrap:anywhere}.pin-error button{display:block;margin-top:5px;background:transparent;border:0;color:#fff;text-decoration:underline;cursor:pointer}.pin-demo-note{position:absolute;bottom:12px;left:50%;transform:translateX(-50%);font-size:11px;padding:7px 10px;background:#131a2bed;color:#b6c2d6;border:1px solid #ffffff18;border-radius:6px;white-space:nowrap;pointer-events:none}@media(max-width:410px){.pin-opacity{display:none}.pin-mark{display:none}.pin-toolbar{gap:0}.pin-toolbar>i{margin:0 2px}}@media(max-width:230px){.pin-toolbar>i,.pin-toolbar button[aria-label="水平翻转图片"],.pin-toolbar button[aria-label="向右旋转图片"]{display:none}.pin-toolbar{padding:3px}.pin-toolbar button{min-width:22px}}
</style>
