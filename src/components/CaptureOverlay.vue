<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { detectRegions } from '../geometry'

type Rect = { x: number; y: number; width: number; height: number }
type Region = Rect & { label: string }
type Frame = { dataUrl: string; width: number; height: number; regions: Region[]; scaleFactor: number }
type Handle = 'n' | 's' | 'e' | 'w' | 'nw' | 'ne' | 'sw' | 'se'
const emit = defineEmits<{
  captured: [image: { dataUrl: string; width: number; height: number; name: string }]
  cancel: []
}>()
const props = withDefaults(defineProps<{ demo?: boolean }>(), { demo: false })
const desktop = !props.demo && '__TAURI_INTERNALS__' in window
const stage = ref<HTMLElement>()
const magnifier = ref<HTMLCanvasElement>()
const frame = ref<Frame>()
const selected = ref<Rect>()
const hovered = ref<Region>()
const pointer = ref({ x: 0, y: 0, clientX: 0, clientY: 0 })
const viewport = ref({ width: window.innerWidth, height: window.innerHeight })
const dragging = ref(false)
const busy = ref(false)
const ready = ref(false)
const error = ref('')
const pixelColor = ref('#000000')
const hideMagnifier = ref(false)
const shiftHeld = ref(false)
const activeHandle = ref<Handle | 'move' | 'new'>('new')
const dragOrigin = ref({ x: 0, y: 0 })
let originalRect: Rect | undefined
let source: HTMLCanvasElement | undefined
let sourceContext: CanvasRenderingContext2D | null = null
let mounted = true
const handles: Handle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']
const currentRect = computed(() => selected.value ?? hovered.value)
const candidateLabel = computed(() => selected.value ? '已选区域' : hovered.value?.label ?? '拖动选择截图区域')
const validSelection = computed(() => ready.value && !!currentRect.value && currentRect.value.width > 0 && currentRect.value.height > 0)
const selectionStyle = computed(() => {
  const r = currentRect.value
  const f = frame.value
  return r && f ? { left: `${r.x / f.width * 100}%`, top: `${r.y / f.height * 100}%`, width: `${r.width / f.width * 100}%`, height: `${r.height / f.height * 100}%` } : {}
})
const toolbarStyle = computed(() => {
  const r = currentRect.value
  const f = frame.value
  if (!r || !f) return {}
  const right = (r.x + r.width) / f.width * viewport.value.width
  const bottom = (r.y + r.height) / f.height * viewport.value.height
  const above = r.y / f.height * viewport.value.height - 58
  return { left: `${Math.min(Math.max(12, right - 302), Math.max(12, viewport.value.width - 314))}px`, top: `${bottom + 62 < viewport.value.height ? bottom + 12 : Math.max(12, above)}px` }
})
const magnifierStyle = computed(() => ({
  left: `${Math.min(Math.max(8, pointer.value.clientX + 26), viewport.value.width - 174)}px`,
  top: `${Math.min(Math.max(8, pointer.value.clientY + 26), viewport.value.height - 166)}px`,
}))

function point(event: PointerEvent) {
  const box = stage.value!.getBoundingClientRect()
  const f = frame.value!
  return { x: Math.max(0, Math.min(f.width, Math.floor((event.clientX - box.left) / box.width * f.width))), y: Math.max(0, Math.min(f.height, Math.floor((event.clientY - box.top) / box.height * f.height))) }
}
function bounded(r: Rect): Rect {
  const f = frame.value!
  const x = Math.max(0, Math.min(f.width - 1, Math.round(r.x)))
  const y = Math.max(0, Math.min(f.height - 1, Math.round(r.y)))
  return { x, y, width: Math.max(1, Math.min(f.width - x, Math.round(r.width))), height: Math.max(1, Math.min(f.height - y, Math.round(r.height))) }
}
function candidates(x: number, y: number) {
  return (frame.value?.regions ?? []).filter(r => x >= r.x && y >= r.y && x < r.x + r.width && y < r.y + r.height && r.width > 0 && r.height > 0).sort((a, b) => a.width * a.height - b.width * b.height)
}
function updateHover() {
  hovered.value = shiftHeld.value ? undefined : candidates(pointer.value.x, pointer.value.y)[0]
}
function updateMagnifier() {
  const canvas = magnifier.value
  const f = frame.value
  if (!sourceContext || !source || !canvas || !f) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  const x = Math.min(f.width - 1, pointer.value.x)
  const y = Math.min(f.height - 1, pointer.value.y)
  ctx.fillStyle = '#10141f'
  ctx.fillRect(0, 0, 150, 100)
  ctx.imageSmoothingEnabled = false
  ctx.drawImage(source, x - 7, y - 5, 15, 10, 0, 0, 150, 100)
  ctx.strokeStyle = '#e9ffb3'
  ctx.lineWidth = 1
  ctx.strokeRect(70.5, 50.5, 9, 9)
  ctx.fillStyle = '#e9ffb3'
  ctx.fillRect(74, 0, 1, 45)
  ctx.fillRect(74, 65, 1, 35)
  ctx.fillRect(0, 54, 65, 1)
  ctx.fillRect(85, 54, 65, 1)
  const rgba = sourceContext.getImageData(x, y, 1, 1).data
  pixelColor.value = '#' + [rgba[0]!, rgba[1]!, rgba[2]!].map(v => v.toString(16).padStart(2, '0')).join('').toUpperCase()
}
function move(event: PointerEvent) {
  if (!frame.value || busy.value) return
  const p = point(event)
  pointer.value = { ...p, clientX: event.clientX, clientY: event.clientY }
  shiftHeld.value = event.shiftKey
  if (dragging.value) {
    const dx = p.x - dragOrigin.value.x
    const dy = p.y - dragOrigin.value.y
    if (activeHandle.value === 'new') {
      selected.value = bounded({ x: Math.min(p.x, dragOrigin.value.x), y: Math.min(p.y, dragOrigin.value.y), width: Math.abs(dx) + 1, height: Math.abs(dy) + 1 })
    } else if (originalRect) {
      if (activeHandle.value === 'move') {
        selected.value = { ...originalRect, x: Math.max(0, Math.min(frame.value.width - originalRect.width, originalRect.x + dx)), y: Math.max(0, Math.min(frame.value.height - originalRect.height, originalRect.y + dy)) }
      } else {
        let left = originalRect.x
        let top = originalRect.y
        let right = left + originalRect.width
        let bottom = top + originalRect.height
        if (activeHandle.value.includes('w')) left = Math.min(right - 1, Math.max(0, left + dx))
        if (activeHandle.value.includes('e')) right = Math.max(left + 1, Math.min(frame.value.width, right + dx))
        if (activeHandle.value.includes('n')) top = Math.min(bottom - 1, Math.max(0, top + dy))
        if (activeHandle.value.includes('s')) bottom = Math.max(top + 1, Math.min(frame.value.height, bottom + dy))
        selected.value = bounded({ x: left, y: top, width: right - left, height: bottom - top })
      }
    }
  } else if (!selected.value) updateHover()
  updateMagnifier()
}
function begin(event: PointerEvent, handle: Handle | 'move' | 'new' = 'new') {
  if (!frame.value || busy.value || event.button !== 0) return
  const p = point(event)
  dragging.value = true
  dragOrigin.value = p
  activeHandle.value = handle
  originalRect = selected.value ? { ...selected.value } : undefined
  if (handle === 'new') selected.value = undefined
  stage.value?.setPointerCapture(event.pointerId)
  event.preventDefault()
}
function end(event: PointerEvent) {
  if (!dragging.value) return
  if (!selected.value && hovered.value && !event.shiftKey) selected.value = bounded(hovered.value)
  dragging.value = false
  if (stage.value?.hasPointerCapture(event.pointerId)) stage.value.releasePointerCapture(event.pointerId)
}
function reset() {
  selected.value = undefined
  error.value = ''
  updateHover()
}
async function confirm() {
  const r = currentRect.value
  if (!r || busy.value || !source) return
  selected.value = bounded(r)
  busy.value = true
  error.value = ''
  try {
    if (desktop) {
      await invoke('finish_capture', { rect: selected.value })
    } else {
      const crop = document.createElement('canvas')
      crop.width = selected.value.width
      crop.height = selected.value.height
      const context = crop.getContext('2d')
      if (!context) throw new Error('当前浏览器无法创建画布')
      context.drawImage(source, selected.value.x, selected.value.y, crop.width, crop.height, 0, 0, crop.width, crop.height)
      emit('captured', { dataUrl: crop.toDataURL('image/png'), width: crop.width, height: crop.height, name: '星镜 · 演示截图' })
    }
  } catch (cause) {
    error.value = String(cause)
  } finally {
    busy.value = false
  }
}
async function cancel() {
  if (busy.value) return
  try {
    if (desktop) await invoke('cancel_capture')
    else emit('cancel')
  } catch (cause) { error.value = String(cause) }
}
function keydown(event: KeyboardEvent) {
  if (busy.value) return
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return
  if (event.key === 'Escape') { event.preventDefault(); void cancel(); return }
  if (error.value) return
  if (event.key === 'Enter') {
    if (event.target instanceof HTMLElement && event.target.closest('button')) return
    event.preventDefault(); void confirm(); return
  }
  if (event.key === 'Shift' && !selected.value) { shiftHeld.value = true; updateHover() }
  if (event.key === 'Tab' && !selected.value) {
    event.preventDefault()
    const choices = candidates(pointer.value.x, pointer.value.y)
    const current = choices.indexOf(hovered.value!)
    hovered.value = choices[(current + (event.shiftKey ? choices.length - 1 : 1)) % choices.length]
    return
  }
  if (event.key.startsWith('Arrow') && currentRect.value && frame.value) {
    event.preventDefault()
    const r = { ...currentRect.value }
    const step = event.shiftKey ? 10 : 1
    const dx = event.key === 'ArrowLeft' ? -step : event.key === 'ArrowRight' ? step : 0
    const dy = event.key === 'ArrowUp' ? -step : event.key === 'ArrowDown' ? step : 0
    if (event.altKey) { r.width += dx; r.height += dy }
    else { r.x = Math.max(0, Math.min(frame.value.width - r.width, r.x + dx)); r.y = Math.max(0, Math.min(frame.value.height - r.height, r.y + dy)) }
    selected.value = bounded(r)
  }
}
function keyup(event: KeyboardEvent) {
  if (event.key === 'Shift') { shiftHeld.value = false; if (!selected.value) updateHover() }
}
function resize() { viewport.value = { width: window.innerWidth, height: window.innerHeight } }
onMounted(async () => {
  window.addEventListener('keydown', keydown)
  window.addEventListener('keyup', keyup)
  window.addEventListener('resize', resize)
  try {
    const next = desktop ? await invoke<Frame>('get_capture_frame') : {
      dataUrl: '/demo-workspace.svg', width: 1200, height: 750, scaleFactor: 1,
      regions: [
        { x: 0, y: 0, width: 1200, height: 750, label: '整个演示画面' },
        { x: 210, y: 48, width: 990, height: 702, label: '演示内容区域' },
        { x: 266, y: 239, width: 560, height: 320, label: '演示插画卡片' },
        { x: 850, y: 239, width: 290, height: 151, label: '演示文字卡片' },
        { x: 850, y: 409, width: 290, height: 150, label: '演示配色卡片' },
      ],
    }
    if (!mounted) return
    frame.value = next
    const image = new Image()
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve()
      image.onerror = () => reject(new Error('截图画面加载失败，请取消后重新截图'))
      image.src = next.dataUrl
    })
    if (!mounted) return
    source = document.createElement('canvas')
    source.width = next.width
    source.height = next.height
    sourceContext = source.getContext('2d', { willReadFrequently: true })
    if (!sourceContext) throw new Error('无法读取截图像素')
    sourceContext.drawImage(image, 0, 0, next.width, next.height)
    // 将 SVG 演示图先栅格化，避免其 preserveAspectRatio 留白导致预判框与像素坐标错位。
    if (!desktop) frame.value.dataUrl = source.toDataURL('image/png')
    // 先缩采样，避免为区域预判复制整张高分屏 RGBA。
    const ratio = Math.min(1, 1440 / Math.max(next.width, next.height))
    const probe = document.createElement('canvas')
    probe.width = Math.max(1, Math.round(next.width * ratio))
    probe.height = Math.max(1, Math.round(next.height * ratio))
    const probeContext = probe.getContext('2d', { willReadFrequently: true })
    if (probeContext) {
      probeContext.drawImage(source, 0, 0, probe.width, probe.height)
      const found = detectRegions(probeContext.getImageData(0, 0, probe.width, probe.height).data, probe.width, probe.height)
      next.regions.push(...found.map(region => ({
        ...bounded({ x: region.x * next.width / probe.width, y: region.y * next.height / probe.height, width: region.width * next.width / probe.width, height: region.height * next.height / probe.height }),
        label: '边缘预测区域',
      })))
    }
    ready.value = true
    pointer.value = { x: Math.floor(next.width / 2), y: Math.floor(next.height / 2), clientX: viewport.value.width / 2, clientY: viewport.value.height / 2 }
    updateHover()
    await nextTick()
    updateMagnifier()
  } catch (cause) { if (mounted) error.value = String(cause) }
})
onBeforeUnmount(() => {
  mounted = false
  window.removeEventListener('keydown', keydown)
  window.removeEventListener('keyup', keyup)
  window.removeEventListener('resize', resize)
  source = undefined
  sourceContext = null
})
</script>

<template>
  <main ref="stage" class="capture-stage" aria-label="截图区域选择" :class="{ 'is-dragging': dragging }" @pointermove="move" @pointerdown="begin($event)" @pointerup="end" @pointercancel="end" @contextmenu.prevent="cancel">
    <img v-if="frame" class="capture-source" :src="frame.dataUrl" alt="当前冻结的屏幕画面" draggable="false" />
    <div v-if="!currentRect" class="capture-shade" />
    <div v-if="currentRect" class="capture-selection" :class="{ 'is-selected': selected }" :style="selectionStyle" @pointerdown.stop="selected ? begin($event, 'move') : begin($event)">
      <div class="capture-dimensions" :class="{ 'place-inside': currentRect.y < 38 }"><span>{{ candidateLabel }}</span><strong>{{ currentRect.width }} × {{ currentRect.height }}</strong></div>
      <template v-if="selected && !dragging">
        <button v-for="handle in handles" :key="handle" class="capture-handle" :class="`handle-${handle}`" :aria-label="`调整选区 ${handle}`" tabindex="-1" @pointerdown.stop="begin($event, handle)" />
      </template>
    </div>
    <aside class="capture-hint" @pointerdown.stop @pointerenter="hideMagnifier = true" @pointerleave="hideMagnifier = false">
      <span class="capture-brand">✦ 星镜</span><span>{{ desktop ? '选择要留下的画面' : '原创演示画面 · 不会读取屏幕' }}</span>
      <span class="capture-help"><kbd>Shift</kbd> 手动框选 <b>·</b> <kbd>Tab</kbd> 切换候选 <b>·</b> <kbd>Esc</kbd> 取消</span>
    </aside>
    <div v-show="frame && !selected && !hideMagnifier && !error" class="capture-magnifier" :style="magnifierStyle" aria-hidden="true">
      <canvas ref="magnifier" width="150" height="100" />
      <div><i :style="{ backgroundColor: pixelColor }" /><code>{{ pixelColor }}</code></div>
      <small>{{ pointer.x }}, {{ pointer.y }} px</small>
    </div>
    <div v-if="selected && !dragging" class="capture-toolbar" :style="toolbarStyle" @pointerdown.stop @pointerenter="hideMagnifier = true" @pointerleave="hideMagnifier = false">
      <button type="button" title="重新选择区域" :disabled="busy" @click="reset">重选</button>
      <button type="button" title="取消截图（Esc）" :disabled="busy" @click="cancel">取消</button>
      <button type="button" class="capture-confirm" :disabled="busy || !validSelection" @click="confirm">{{ busy ? '正在截图…' : '✓ 完成截图' }} <kbd>↵</kbd></button>
    </div>
    <div v-if="selected && !dragging" class="capture-keyhint">方向键移动 1 px · Shift 加速 · Alt + 方向键调整尺寸</div>
    <div v-if="error" role="alert" class="capture-error" @pointerdown.stop><strong>暂时无法完成截图</strong><p>{{ error }}</p><button type="button" @click="cancel">取消并返回</button></div>
    <div v-if="!frame && !error" class="capture-loading" role="status">正在准备截图画面…</div>
  </main>
</template>

<style scoped>
.capture-stage{position:fixed;inset:0;background:#0b0e16;overflow:hidden;color:#f2f4fa;font-family:Inter,"PingFang SC","Microsoft YaHei",sans-serif;cursor:crosshair;user-select:none;touch-action:none}
.capture-stage button{font:inherit}.capture-source{position:absolute;inset:0;width:100%;height:100%;object-fit:fill;pointer-events:none}.capture-shade{position:absolute;inset:0;background:#06091470;pointer-events:none}.capture-selection{position:absolute;box-sizing:border-box;border:1px solid #d9f99d;box-shadow:0 0 0 9999px #05081385;z-index:1}.capture-selection.is-selected{cursor:move}.capture-dimensions{position:absolute;left:-1px;bottom:calc(100% + 9px);display:flex;align-items:center;gap:12px;max-width:calc(100vw - 24px);padding:7px 10px;background:#171c2cef;border:1px solid #ffffff20;border-radius:7px;white-space:nowrap;font-size:11px;pointer-events:none}.capture-dimensions.place-inside{bottom:auto;top:8px;left:8px}.capture-dimensions span{color:#bbc3d4}.capture-dimensions strong{font-size:12px;font-weight:600;color:#e8ffbc;font-variant-numeric:tabular-nums}.capture-handle{position:absolute;width:9px;height:9px;padding:0;border:1px solid #11182c;background:#e1fda6;border-radius:2px}.handle-nw{top:-5px;left:-5px;cursor:nwse-resize}.handle-ne{top:-5px;right:-5px;cursor:nesw-resize}.handle-sw{bottom:-5px;left:-5px;cursor:nesw-resize}.handle-se{bottom:-5px;right:-5px;cursor:nwse-resize}.handle-n{top:-5px;left:calc(50% - 4px);cursor:ns-resize}.handle-s{bottom:-5px;left:calc(50% - 4px);cursor:ns-resize}.handle-w{left:-5px;top:calc(50% - 4px);cursor:ew-resize}.handle-e{right:-5px;top:calc(50% - 4px);cursor:ew-resize}.capture-hint{position:absolute;z-index:3;top:18px;left:50%;transform:translateX(-50%);display:flex;gap:16px;align-items:center;max-width:calc(100vw - 36px);padding:11px 17px;border:1px solid #ffffff1f;border-radius:11px;background:#111725ed;box-shadow:0 8px 35px #0003;cursor:default;font-size:12px;white-space:nowrap}.capture-brand{font-weight:700;color:#dff6a6;letter-spacing:1px}.capture-hint>span:nth-child(2){color:#e1e5ee}.capture-help{color:#98a4bb;font-size:11px}.capture-help b{font-weight:400;margin:0 7px;color:#526079}kbd{font-family:inherit;font-size:10px;padding:2px 5px;background:#ffffff0a;border:1px solid #ffffff1f;border-radius:4px}.capture-magnifier{position:absolute;z-index:4;width:150px;padding:5px;border:1px solid #ffffff34;border-radius:9px;background:#101625f5;box-shadow:0 8px 26px #0005;pointer-events:none}.capture-magnifier canvas{display:block;width:150px;height:100px;border-radius:4px}.capture-magnifier div{display:flex;align-items:center;justify-content:center;gap:7px;padding-top:7px}.capture-magnifier i{width:12px;height:12px;border-radius:3px;border:1px solid #ffffff25}.capture-magnifier code{font-size:11px;color:#e3e9f3}.capture-magnifier small{display:block;text-align:center;font-size:10px;color:#8490a8;padding:4px 0 2px;font-variant-numeric:tabular-nums}.capture-toolbar{position:absolute;z-index:5;display:flex;align-items:center;gap:5px;padding:6px;background:#151b2bf5;border:1px solid #ffffff25;box-shadow:0 8px 32px #0005;border-radius:10px;cursor:default}.capture-toolbar button,.capture-error button{background:transparent;border:0;color:#d6deed;cursor:pointer;border-radius:6px;padding:9px 12px;font-size:12px;white-space:nowrap}.capture-toolbar button:hover{background:#ffffff10}.capture-toolbar button.capture-confirm{background:#d9f6a1;color:#182317;font-weight:650}.capture-toolbar .capture-confirm:hover{background:#e7ffbe}.capture-confirm kbd{margin-left:10px;border-color:#1b271526;background:#192b0b0d}.capture-toolbar button:disabled{opacity:.5;cursor:wait}.capture-toolbar button:focus-visible,.capture-error button:focus-visible{outline:2px solid #e8ffb6;outline-offset:3px}.capture-keyhint{position:absolute;z-index:3;bottom:15px;left:50%;transform:translateX(-50%);padding:7px 13px;background:#111725dc;border:1px solid #ffffff12;border-radius:7px;color:#a6b0c5;font-size:11px;white-space:nowrap;pointer-events:none}.capture-error{position:absolute;z-index:8;top:50%;left:50%;transform:translate(-50%,-50%);width:min(380px,calc(100vw - 48px));padding:25px;background:#151c2c;border:1px solid #ffb4ac66;border-radius:14px;box-shadow:0 20px 80px #0008;cursor:default}.capture-error strong{color:#ffd0cb;font-size:15px}.capture-error p{font-size:13px;line-height:1.7;color:#abb6cb;overflow-wrap:anywhere}.capture-error button{background:#ffffff10;width:100%}.capture-loading{position:absolute;inset:0;display:grid;place-items:center;font-size:14px;color:#aebbd0}.is-dragging .capture-selection{cursor:crosshair}@media(max-width:850px){.capture-help{display:none}.capture-hint{top:10px;padding:9px 13px}.capture-keyhint{font-size:10px}}
</style>
