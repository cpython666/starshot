<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

const desktop = '__TAURI_INTERNALS__' in window
const image = ref<{ dataUrl: string; width: number; height: number }>()
const zoom = ref(1)
const opacity = ref(1)
const rotation = ref(0)
const flipped = ref(false)
const toolbarVisible = ref(true)
const error = ref('')
const viewport = ref({ width: window.innerWidth, height: window.innerHeight })
let unlisten: UnlistenFn | undefined
let mounted = true
let scaleQueue = Promise.resolve()
let confirmedZoom = 1
const imageStyle = computed(() => ({
  width: `${rotation.value % 180 ? viewport.value.height : viewport.value.width}px`,
  height: `${rotation.value % 180 ? viewport.value.width : viewport.value.height}px`,
  transform: `rotate(${rotation.value}deg) scale(${desktop ? 1 : zoom.value}) scaleX(${flipped.value ? -1 : 1})`,
}))
async function action(actionName: string, value?: number) {
  if (!desktop) return true
  try { await invoke('pin_action', { action: actionName, value }); return true }
  catch (cause) { error.value = String(cause); return false }
}
function drag(event: PointerEvent) {
  if (event.button !== 0 || !desktop) return
  // 原生拖动必须直接由按下事件发起，等待移动或长按会错失 macOS 起拖时机。
  void action('drag')
}
function setZoom(value: number) {
  const next = Math.max(.1, Math.min(4, Math.round(value * 100) / 100))
  zoom.value = next
  if (desktop) scaleQueue = scaleQueue.then(async () => {
    if (await action('scale', next)) confirmedZoom = next
    else if (zoom.value === next) zoom.value = confirmedZoom
  })
}
function wheel(event: WheelEvent) {
  if (!event.deltaY) return
  event.preventDefault()
  if (event.ctrlKey || event.metaKey) setOpacity(opacity.value + (event.deltaY < 0 ? .05 : -.05))
  else setZoom(zoom.value * (event.deltaY < 0 ? 1.1 : 1 / 1.1))
}
function setOpacity(value: number) {
  opacity.value = Math.max(.15, Math.min(1, Math.round(value * 100) / 100))
  void action('opacity', opacity.value)
}
function reset() { rotation.value = 0; flipped.value = false; setZoom(1); setOpacity(1) }
function resized() { viewport.value = { width: window.innerWidth, height: window.innerHeight } }
function close() {
  if (desktop) void action('close')
  else window.location.assign('/')
}
function keydown(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); close(); return }
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
  try {
    if (desktop) {
      const loaded = await invoke<{ dataUrl: string; width: number; height: number }>('get_pin_image')
      if (!mounted) return
      image.value = loaded
      const off = await listen<number>('pin-opacity', event => { opacity.value = event.payload })
      if (!mounted) off()
      else unlisten = off
    } else image.value = { dataUrl: '/demo-workspace.svg', width: 1200, height: 750 }
  } catch (cause) { if (mounted) error.value = String(cause) }
})
onBeforeUnmount(() => { mounted = false; unlisten?.(); window.removeEventListener('keydown', keydown); window.removeEventListener('resize', resized); document.documentElement.classList.remove('starshot-pin-document') })
</script>

<template>
  <main class="pin-window" aria-label="星镜置顶贴图" @wheel="wheel">
    <div class="pin-image-area" :style="{ opacity }" @pointerdown="drag" @dblclick="toolbarVisible = !toolbarVisible">
      <img v-if="image" :src="image.dataUrl" :style="imageStyle" draggable="false" alt="置顶参考图片" />
      <span v-else-if="!error" class="pin-loading">正在加载贴图…</span>
    </div>
    <div v-show="toolbarVisible" class="pin-toolbar" aria-label="贴图工具" @pointerdown.stop>
      <span class="pin-mark" title="星镜置顶贴图">✦</span>
      <button type="button" aria-label="缩小贴图" title="缩小（−）" @click="setZoom(zoom / 1.1)">−</button>
      <button type="button" class="pin-scale" title="恢复初始大小和方向（0）" @click="reset">{{ Math.round(zoom * 100) }}%</button>
      <button type="button" aria-label="放大贴图" title="放大（+）" @click="setZoom(zoom * 1.1)">+</button>
      <i />
      <button type="button" aria-label="向右旋转图片" title="旋转 90°（R）" @click="rotation = (rotation + 90) % 360">↻</button>
      <button type="button" aria-label="水平翻转图片" title="水平翻转（F）" @click="flipped = !flipped">↔</button>
      <label class="pin-opacity" title="贴图不透明度"><span>◐</span><input aria-label="贴图不透明度" type="range" min="0.15" max="1" step="0.05" :value="opacity" @input="setOpacity(Number(($event.target as HTMLInputElement).value))" /></label>
      <i />
      <button v-if="desktop" type="button" aria-label="开启鼠标穿透" title="开启鼠标穿透；从托盘「恢复所有贴图鼠标操作」关闭穿透" @click="action('ignore-cursor',1)">⇥</button>
      <button type="button" aria-label="隐藏贴图工具栏" title="隐藏工具栏（空格）；双击图片可恢复" @click="toolbarVisible = false">⌄</button>
      <button type="button" class="pin-close" aria-label="关闭贴图" title="关闭贴图（Esc）" @click="close">×</button>
    </div>
    <button v-if="!toolbarVisible" class="pin-reveal" aria-label="显示贴图工具栏" title="显示工具栏（空格）" @click="toolbarVisible = true">✦</button>
    <div v-if="error" role="alert" class="pin-error">{{ error }}<button type="button" @click="error = ''">关闭提示</button></div>
    <span v-if="!desktop" class="pin-demo-note">浏览器演示 · 桌面版支持置顶与拖动</span>
  </main>
</template>

<style>
html.starshot-pin-document,html.starshot-pin-document body,html.starshot-pin-document #app{background:transparent!important;margin:0;width:100%;height:100%;overflow:hidden}
</style>
<style scoped>
.pin-window{position:fixed;inset:0;color:#edf2fc;font:12px Inter,"PingFang SC","Microsoft YaHei",sans-serif;user-select:none;overflow:hidden;background:transparent}.pin-image-area{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;cursor:grab;overflow:hidden}.pin-image-area:active{cursor:grabbing}.pin-image-area img{display:block;max-width:none;max-height:none;flex-shrink:0;object-fit:contain;pointer-events:none;transform-origin:center}.pin-toolbar{position:absolute;z-index:2;display:flex;align-items:center;justify-content:center;gap:2px;left:50%;top:9px;transform:translateX(-50%);padding:5px;border:1px solid #ffffff24;border-radius:9px;background:#121827ed;box-shadow:0 5px 24px #0004;white-space:nowrap;max-width:calc(100vw - 16px)}.pin-toolbar button{display:grid;place-items:center;flex-shrink:0;min-width:25px;height:28px;padding:0 5px;color:#dbe5f5;background:none;border:0;border-radius:5px;font-family:inherit;font-size:16px;cursor:pointer}.pin-toolbar button:hover{background:#ffffff18}.pin-toolbar button:focus-visible,.pin-reveal:focus-visible{outline:2px solid #dff5a4;outline-offset:2px}.pin-mark{display:block;padding:0 6px;color:#dff5a4;font-size:16px}.pin-toolbar .pin-scale{font-size:10px;font-variant-numeric:tabular-nums;min-width:40px}.pin-toolbar>i{width:1px;height:17px;background:#ffffff17;margin:0 4px}.pin-opacity{display:flex;gap:4px;align-items:center;padding:0 3px;color:#b4c0d6}.pin-opacity input{width:55px;accent-color:#dff5a4;cursor:pointer}.pin-close:hover{color:#ffbbb3!important}.pin-reveal{position:absolute;right:7px;top:7px;border:1px solid #ffffff35;border-radius:6px;background:#151c2cd9;color:#dff5a4;width:26px;height:26px;cursor:pointer;opacity:.25}.pin-reveal:hover,.pin-reveal:focus-visible{opacity:1}.pin-loading{font-size:12px;color:#aab5cb;background:#171e2c;padding:12px 18px;border-radius:9px}.pin-error{position:absolute;bottom:9px;left:9px;right:9px;padding:10px;border:1px solid #eaa9a94d;border-radius:8px;background:#341e28f5;color:#ffd1cc;font-size:11px;line-height:1.6;overflow-wrap:anywhere}.pin-error button{display:block;margin-top:5px;background:transparent;border:0;color:#fff;text-decoration:underline;cursor:pointer}.pin-demo-note{position:absolute;bottom:12px;left:50%;transform:translateX(-50%);font-size:11px;padding:7px 10px;background:#131a2bed;color:#b6c2d6;border:1px solid #ffffff18;border-radius:6px;white-space:nowrap;pointer-events:none}@media(max-width:410px){.pin-opacity{display:none}.pin-mark{display:none}.pin-toolbar{gap:0}.pin-toolbar>i{margin:0 2px}}@media(max-width:230px){.pin-toolbar>i,.pin-toolbar button[aria-label="水平翻转图片"],.pin-toolbar button[aria-label="向右旋转图片"]{display:none}.pin-toolbar{padding:3px}.pin-toolbar button{min-width:22px}}
</style>
