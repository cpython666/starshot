<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { captureRegionsAt, normalizeRect } from '../geometry'
import { DEFAULT_STYLE, renderImage, renderPreview } from '../editor'
import type { Annotation } from '../editor'
import Icon from './Icon.vue'

type Rect = { x: number; y: number; width: number; height: number }
type Region = Rect & { label: string }
type Frame = { dataUrl: string; width: number; height: number; regions: Region[]; scaleFactor: number }
type Handle = 'n' | 's' | 'e' | 'w' | 'nw' | 'ne' | 'sw' | 'se'
type Action = 'copy' | 'save' | 'pin' | 'edit'
const emit = defineEmits<{ captured: [image: { dataUrl: string; width: number; height: number; name: string }]; cancel: [] }>()
const props = withDefaults(defineProps<{ demo?: boolean }>(), { demo: false })
const desktop = !props.demo && '__TAURI_INTERNALS__' in window
const stage = ref<HTMLElement>()
const magnifier = ref<HTMLCanvasElement>()
const preview = ref<HTMLCanvasElement>()
const toolbar = ref<HTMLElement>()
const frame = shallowRef<Frame>()
const selected = ref<Rect>()
const hovered = ref<Region>()
const pointer = ref({ x: 0, y: 0, clientX: 0, clientY: 0 })
const viewport = ref({ width: window.innerWidth, height: window.innerHeight })
const toolbarSize = ref({ width: 640, height: 48 })
const dragging = ref(false)
const busy = ref(false)
const ready = ref(false)
const error = ref('')
const status = ref('')
const pixelColor = ref('#000000')
const hideMagnifier = ref(false)
const shiftHeld = ref(false)
const activeHandle = ref<Handle | 'move' | 'new'>('new')
const dragOrigin = ref({ x: 0, y: 0 })
const tool = ref('cursor')
const color = ref('#f43f5e')
const lineWidth = ref(3)
const text = ref('')
const annotations = shallowRef<Annotation[]>([])
const draft = shallowRef<Annotation>()
const undoStack: Annotation[][] = []
const redoStack: Annotation[][] = []
const historyRevision = ref(0)
const canUndo = computed(() => { void historyRevision.value; return undoStack.length > 0 })
const canRedo = computed(() => { void historyRevision.value; return redoStack.length > 0 })
const tools = [
  { id: 'cursor', label: '调整选区', key: 'V' }, { id: 'rect', label: '矩形', key: 'R' },
  { id: 'ellipse', label: '椭圆', key: 'O' }, { id: 'arrow', label: '箭头', key: 'A' },
  { id: 'pen', label: '画笔', key: 'P' }, { id: 'text', label: '文字', key: 'T' },
  { id: 'mosaic', label: '马赛克', key: 'M' }, { id: 'redact', label: '实心遮盖', key: 'B' },
]
const flatStyle = { ...DEFAULT_STYLE, background: 'transparent' as const, padding: 0, radius: 0, borderWidth: 0, shadow: 0 }
let originalRect: Rect | undefined
let source: HTMLImageElement | HTMLCanvasElement | undefined
let regions: Region[] = []
let windowRegions: Region[] = []
let mounted = true
let moveRequest = 0
let drawRequest = 0
let probeRequest = 0
let pendingMove: PointerEvent | undefined
let annotationOrigin: { x: number; y: number } | undefined
let pointerStart = { x: 0, y: 0 }
let regionWorker: Worker | undefined
let toolbarObserver: ResizeObserver | undefined
const handles: Handle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']
const currentRect = computed(() => selected.value ?? hovered.value)
const validSelection = computed(() => ready.value && !!currentRect.value && currentRect.value.width > 0 && currentRect.value.height > 0)
const selectionStyle = computed(() => {
  const r = currentRect.value, f = frame.value
  return r && f ? { left: `${r.x / f.width * 100}%`, top: `${r.y / f.height * 100}%`, width: `${r.width / f.width * 100}%`, height: `${r.height / f.height * 100}%` } : {}
})
const toolbarStyle = computed(() => {
  const r = currentRect.value, f = frame.value
  if (!r || !f) return {}
  const right = (r.x + r.width) / f.width * viewport.value.width
  const bottom = (r.y + r.height) / f.height * viewport.value.height
  const top = r.y / f.height * viewport.value.height
  const { width, height } = toolbarSize.value
  const y = bottom + height + 16 <= viewport.value.height ? bottom + 8 : top >= height + 16 ? top - height - 8 : Math.max(8, Math.min(bottom - height - 8, viewport.value.height - height - 8))
  return { left: `${Math.max(8, Math.min(right - width, viewport.value.width - width - 8))}px`, top: `${y}px` }
})
const magnifierStyle = computed(() => ({
  left: `${Math.max(8, Math.min(pointer.value.clientX + 26, viewport.value.width - 174))}px`,
  top: `${Math.max(8, Math.min(pointer.value.clientY + 26, viewport.value.height - 166))}px`,
}))
const showMagnifier = computed(() => ready.value && !hideMagnifier.value && !busy.value && !error.value && (!selected.value || dragging.value))

function point(event: PointerEvent) {
  const f = frame.value!
  return { x: Math.max(0, Math.min(f.width - 1, Math.floor(event.clientX / viewport.value.width * f.width))), y: Math.max(0, Math.min(f.height - 1, Math.floor(event.clientY / viewport.value.height * f.height))) }
}
function bounded(r: Rect): Rect {
  const f = frame.value!
  const x = Math.max(0, Math.min(f.width - 1, Math.round(r.x)))
  const y = Math.max(0, Math.min(f.height - 1, Math.round(r.y)))
  return { x, y, width: Math.max(1, Math.min(f.width - x, Math.round(r.width))), height: Math.max(1, Math.min(f.height - y, Math.round(r.height))) }
}
const contains = (r: Rect, x: number, y: number) => x >= r.x && y >= r.y && x < r.x + r.width && y < r.y + r.height
function updateHover() {
  hovered.value = shiftHeld.value || !frame.value ? undefined : captureRegionsAt(pointer.value, windowRegions, regions, frame.value)[0]
}
function updateMagnifier() {
  if (!showMagnifier.value || !source || !magnifier.value) return
  const ctx = magnifier.value.getContext('2d', { willReadFrequently: true })
  if (!ctx) return
  const { x, y } = pointer.value
  ctx.fillStyle = '#10141f'; ctx.fillRect(0, 0, 150, 100)
  ctx.imageSmoothingEnabled = false
  ctx.drawImage(source, x - 7, y - 5, 15, 10, 0, 0, 150, 100)
  // Read the tiny magnifier, not a second full-screen CPU canvas.
  const rgba = ctx.getImageData(75, 55, 1, 1).data
  pixelColor.value = '#' + [rgba[0]!, rgba[1]!, rgba[2]!].map(v => v.toString(16).padStart(2, '0')).join('').toUpperCase()
  ctx.strokeStyle = '#e9ffb3'; ctx.lineWidth = 1; ctx.strokeRect(70.5, 50.5, 9, 9)
}
function checkpoint() {
  undoStack.push(annotations.value); if (undoStack.length > 60) undoStack.shift()
  redoStack.length = 0; historyRevision.value++
}
function undo() {
  if (busy.value || annotationOrigin || dragging.value || !undoStack.length) return
  redoStack.push(annotations.value); annotations.value = undoStack.pop()!; historyRevision.value++
}
function redo() {
  if (busy.value || annotationOrigin || dragging.value || !redoStack.length) return
  undoStack.push(annotations.value); annotations.value = redoStack.pop()!; historyRevision.value++
}
function scheduleDraw() {
  if (drawRequest) return
  drawRequest = requestAnimationFrame(() => { drawRequest = 0; drawPreview() })
}
function drawPreview() {
  const r = selected.value, f = frame.value, canvas = preview.value
  if (!r || !f || !source || !canvas) return
  if (!annotations.value.length && !draft.value) { canvas.width = 1; canvas.height = 1; return }
  try {
    const item = draft.value
    const items = item && (item.type !== 'redact' || item.width > 0 && item.height > 0) ? [...annotations.value, item] : annotations.value
    const ratio = Math.min(window.devicePixelRatio || 1, 2)
    const result = renderPreview(source, f, items, flatStyle, { crop: r }, { width: r.width / f.width * viewport.value.width * ratio, height: r.height / f.height * viewport.value.height * ratio })
    if (canvas.width !== result.width) canvas.width = result.width
    if (canvas.height !== result.height) canvas.height = result.height
    const ctx = canvas.getContext('2d')!
    ctx.clearRect(0, 0, canvas.width, canvas.height); ctx.drawImage(result, 0, 0)
  } catch (cause) { error.value = String(cause) }
}
function applyMove(event: PointerEvent) {
  if (!ready.value || !frame.value || busy.value) return
  const p = point(event)
  const moved = p.x !== pointer.value.x || p.y !== pointer.value.y || event.shiftKey !== shiftHeld.value
  pointer.value = { ...p, clientX: event.clientX, clientY: event.clientY }
  shiftHeld.value = event.shiftKey
  if (annotationOrigin && draft.value) {
    const r = selected.value!
    p.x = Math.max(r.x, Math.min(r.x + r.width - 1, p.x)); p.y = Math.max(r.y, Math.min(r.y + r.height - 1, p.y))
    if (draft.value.type === 'pen') {
      const points = draft.value.points!
      if (points.length < 20_000 && (points.at(-1)!.x !== p.x || points.at(-1)!.y !== p.y)) draft.value = { ...draft.value, points: [...points, p] }
    } else if (draft.value.type === 'arrow') draft.value = { ...draft.value, width: p.x - annotationOrigin.x, height: p.y - annotationOrigin.y }
    else draft.value = { ...draft.value, ...normalizeRect(annotationOrigin, p) }
    return
  }
  if (dragging.value) {
    const dx = p.x - dragOrigin.value.x, dy = p.y - dragOrigin.value.y
    if (activeHandle.value === 'new') {
      if (Math.hypot(event.clientX - pointerStart.x, event.clientY - pointerStart.y) >= 3) selected.value = bounded({ x: Math.min(p.x, dragOrigin.value.x), y: Math.min(p.y, dragOrigin.value.y), width: Math.abs(dx) + 1, height: Math.abs(dy) + 1 })
    } else if (originalRect) {
      if (activeHandle.value === 'move') selected.value = { ...originalRect, x: Math.max(0, Math.min(frame.value.width - originalRect.width, originalRect.x + dx)), y: Math.max(0, Math.min(frame.value.height - originalRect.height, originalRect.y + dy)) }
      else {
        let left = originalRect.x, top = originalRect.y, right = left + originalRect.width, bottom = top + originalRect.height
        if (activeHandle.value.includes('w')) left = Math.min(right - 1, Math.max(0, left + dx))
        if (activeHandle.value.includes('e')) right = Math.max(left + 1, Math.min(frame.value.width, right + dx))
        if (activeHandle.value.includes('n')) top = Math.min(bottom - 1, Math.max(0, top + dy))
        if (activeHandle.value.includes('s')) bottom = Math.max(top + 1, Math.min(frame.value.height, bottom + dy))
        selected.value = bounded({ x: left, y: top, width: right - left, height: bottom - top })
      }
    }
  } else if (!selected.value && moved) updateHover()
  updateMagnifier()
}
function move(event: PointerEvent) {
  pendingMove = event
  if (moveRequest) return
  moveRequest = requestAnimationFrame(() => { moveRequest = 0; const event = pendingMove; pendingMove = undefined; if (event) applyMove(event) })
}
function flushMove(event?: PointerEvent) {
  cancelAnimationFrame(moveRequest); moveRequest = 0
  const latest = event ?? pendingMove; pendingMove = undefined
  if (latest) applyMove(latest)
}
function begin(event: PointerEvent, handle: Handle | 'move' | 'new' = 'new') {
  if (!ready.value || busy.value || event.button !== 0) return
  flushMove(event)
  const p = point(event)
  error.value = ''; status.value = ''
  if (selected.value && tool.value !== 'cursor' && contains(selected.value, p.x, p.y)) {
    if (annotations.value.length >= 1_000) { error.value = '标注过多，请先撤销或到更多编辑整理。'; return }
    const item: Annotation = { id: crypto.randomUUID(), type: tool.value as Annotation['type'], ...p, width: 0, height: 0, color: color.value, lineWidth: lineWidth.value, fontSize: 26, points: [p] }
    if (tool.value === 'text') {
      if (!text.value.trim()) { status.value = '先输入文字，再点击选区放置。'; return }
      item.text = text.value.slice(0, 1_000); checkpoint(); annotations.value = [...annotations.value, item]
    } else { annotationOrigin = p; draft.value = item }
    stage.value?.setPointerCapture(event.pointerId); event.preventDefault(); return
  }
  // Clicking outside an established selection must not discard annotations.
  if (selected.value && handle === 'new') return
  dragging.value = true; dragOrigin.value = p; pointerStart = { x: event.clientX, y: event.clientY }
  activeHandle.value = handle; originalRect = selected.value ? { ...selected.value } : undefined
  stage.value?.setPointerCapture(event.pointerId); event.preventDefault()
}
function end(event: PointerEvent) {
  flushMove(event)
  if (annotationOrigin && draft.value) {
    const item = draft.value
    if (item.type === 'pen' || (item.type === 'arrow' ? Math.abs(item.width) + Math.abs(item.height) > 2 : item.width > 1 && item.height > 1)) {
      checkpoint(); annotations.value = [...annotations.value, item]
    }
    annotationOrigin = undefined; draft.value = undefined
  } else if (dragging.value && !selected.value) {
    if (hovered.value && !event.shiftKey) selected.value = bounded(hovered.value)
  }
  dragging.value = false
  if (stage.value?.hasPointerCapture(event.pointerId)) stage.value.releasePointerCapture(event.pointerId)
}
function abortPointer() {
  cancelAnimationFrame(moveRequest); moveRequest = 0; pendingMove = undefined
  annotationOrigin = undefined; draft.value = undefined
  if (dragging.value && originalRect) selected.value = originalRect
  dragging.value = false
}
function reset() {
  if (busy.value) return
  abortPointer(); selected.value = undefined; annotations.value = []; undoStack.length = 0; redoStack.length = 0; historyRevision.value++
  tool.value = 'cursor'; error.value = ''; status.value = ''; updateHover()
}
function setTool(id: string) { if (!busy.value) { abortPointer(); tool.value = id; error.value = ''; status.value = '' } }
async function perform(action: Action) {
  flushMove()
  const r = currentRect.value, f = frame.value
  if (!r || !f || !source || busy.value || !ready.value || dragging.value || annotationOrigin) return
  selected.value = bounded(r); busy.value = true; error.value = ''; status.value = ''
  try {
    // Unannotated native captures stay RGBA in Rust for save/copy; only edits cross IPC.
    if (desktop) {
      const dataUrl = annotations.value.length ? renderImage(source, f, annotations.value, flatStyle, { crop: selected.value }).toDataURL('image/png') : undefined
      const result = await invoke<{ completed: boolean; path?: string }>('finish_capture', { rect: selected.value, action, dataUrl })
      if (!result.completed) status.value = '已取消保存，选区和标注保留。'
    } else {
      const result = renderImage(source, f, annotations.value, flatStyle, { crop: selected.value })
      if (action === 'edit') emit('captured', { dataUrl: result.toDataURL('image/png'), width: result.width, height: result.height, name: '星镜 · 演示截图' })
      else if (action === 'pin') throw new Error('贴到桌面仅在桌面版中可用')
      else {
        const blob = await new Promise<Blob>((resolve, reject) => result.toBlob(blob => blob ? resolve(blob) : reject(new Error('图片编码失败')), 'image/png'))
        if (action === 'copy') {
          if (!navigator.clipboard?.write || typeof ClipboardItem === 'undefined') throw new Error('此浏览器不支持复制图片，请保存 PNG')
          await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]); status.value = '图片已复制。'
        } else {
          const url = URL.createObjectURL(blob), link = document.createElement('a')
          link.href = url; link.download = '星镜-截图.png'; link.click(); setTimeout(() => URL.revokeObjectURL(url), 30_000)
          status.value = '已发起 PNG 下载。'
        }
      }
    }
  } catch (cause) { error.value = String(cause) }
  finally { busy.value = false }
}
async function cancel() {
  if (busy.value) return
  abortPointer(); busy.value = true
  try { if (desktop) await invoke('cancel_capture'); else emit('cancel') }
  catch (cause) { error.value = String(cause) }
  finally { busy.value = false }
}
function keydown(event: KeyboardEvent) {
  if (busy.value) { event.preventDefault(); return }
  const input = event.target instanceof HTMLElement && event.target.closest('input,textarea,select')
  if (input) { if (event.key === 'Escape') { event.preventDefault(); (event.target as HTMLElement).blur() }; return }
  if (event.key === 'Escape') { event.preventDefault(); if (annotationOrigin) abortPointer(); else void cancel(); return }
  if (event.metaKey || event.ctrlKey) {
    const key = event.key.toLowerCase()
    if (key === 'z') { event.preventDefault(); event.shiftKey ? redo() : undo() }
    if (key === 'c' || key === 's') { event.preventDefault(); void perform(key === 'c' ? 'copy' : 'save') }
    return
  }
  if (event.key === 'Enter') {
    if (event.target instanceof HTMLElement && event.target.closest('button')) return
    event.preventDefault(); void perform('copy'); return
  }
  if (event.key === 'Shift' && !selected.value) { shiftHeld.value = true; updateHover() }
  if (event.key === 'Tab' && !selected.value) {
    event.preventDefault()
    const choices = frame.value ? captureRegionsAt(pointer.value, windowRegions, regions, frame.value) : []
    const index = choices.findIndex(r => r.x === hovered.value?.x && r.y === hovered.value?.y && r.width === hovered.value?.width && r.height === hovered.value?.height)
    if (choices.length) hovered.value = choices[index < 0 ? (event.shiftKey ? choices.length - 1 : 0) : (index + (event.shiftKey ? choices.length - 1 : 1)) % choices.length]
    return
  }
  if (selected.value) {
    const next = tools.find(t => t.key.toLowerCase() === event.key.toLowerCase())
    if (next) { event.preventDefault(); setTool(next.id); return }
  }
  if (event.key.startsWith('Arrow') && currentRect.value && frame.value && tool.value === 'cursor') {
    event.preventDefault()
    const r = { ...currentRect.value }, step = event.shiftKey ? 10 : 1
    const dx = event.key === 'ArrowLeft' ? -step : event.key === 'ArrowRight' ? step : 0
    const dy = event.key === 'ArrowUp' ? -step : event.key === 'ArrowDown' ? step : 0
    if (event.altKey) { r.width += dx; r.height += dy }
    else { r.x = Math.max(0, Math.min(frame.value.width - r.width, r.x + dx)); r.y = Math.max(0, Math.min(frame.value.height - r.height, r.y + dy)) }
    selected.value = bounded(r)
  }
}
function keyup(event: KeyboardEvent) { if (event.key === 'Shift') { shiftHeld.value = false; if (!selected.value) updateHover() } }
function resize() { viewport.value = { width: window.innerWidth, height: window.innerHeight }; scheduleDraw() }
function findEdges() {
  if (!source || !frame.value || !mounted) return
  const f = frame.value, ratio = Math.min(1, 1920 / Math.max(f.width, f.height))
  const probe = document.createElement('canvas')
  probe.width = Math.max(1, Math.round(f.width * ratio)); probe.height = Math.max(1, Math.round(f.height * ratio))
  const ctx = probe.getContext('2d', { willReadFrequently: true })
  if (!ctx) return
  ctx.drawImage(source, 0, 0, probe.width, probe.height)
  const pixels = ctx.getImageData(0, 0, probe.width, probe.height).data
  const worker = new Worker(new URL('../capture-regions.worker.ts', import.meta.url), { type: 'module' })
  regionWorker = worker
  worker.onmessage = (event: MessageEvent<Region[]>) => {
    if (mounted) {
      regions = event.data.map(region => ({ ...bounded({ x: region.x * f.width / probe.width, y: region.y * f.height / probe.height, width: region.width * f.width / probe.width, height: region.height * f.height / probe.height }), label: '边缘候选' })).sort((a, b) => a.width * a.height - b.width * b.height)
      if (!selected.value && !dragging.value) updateHover()
    }
    worker.terminate(); if (regionWorker === worker) regionWorker = undefined
  }
  worker.onerror = () => { worker.terminate(); if (regionWorker === worker) regionWorker = undefined }
  worker.postMessage({ pixels, width: probe.width, height: probe.height }, [pixels.buffer])
}
watch([annotations, draft, selected], scheduleDraw)
watch(toolbar, element => {
  toolbarObserver?.disconnect()
  if (element) {
    toolbarObserver = new ResizeObserver(([entry]) => { if (entry) toolbarSize.value = { width: element.offsetWidth, height: element.offsetHeight } })
    toolbarObserver.observe(element)
  }
})
onMounted(async () => {
  window.addEventListener('keydown', keydown); window.addEventListener('keyup', keyup); window.addEventListener('resize', resize)
  try {
    const next = desktop ? await invoke<Frame>('get_capture_frame') : { dataUrl: '/demo-workspace.svg', width: 1200, height: 750, scaleFactor: 1, regions: [{ x: 0, y: 0, width: 1200, height: 750, label: '整个演示画面' }] }
    if (!mounted) return
    frame.value = next
    const image = new Image(); image.src = next.dataUrl; await image.decode()
    if (!mounted) return
    source = image
    if (!desktop) {
      const raster = document.createElement('canvas'); raster.width = next.width; raster.height = next.height
      raster.getContext('2d')!.drawImage(image, 0, 0, next.width, next.height)
      source = raster; frame.value = { ...next, dataUrl: raster.toDataURL('image/png') }
    }
    // Preserve the system front-to-back order; hidden smaller windows must not steal hover.
    windowRegions = desktop ? next.regions.filter(r => r.width > 0 && r.height > 0) : []
    ready.value = true
    pointer.value = { x: Math.floor(next.width / 2), y: Math.floor(next.height / 2), clientX: viewport.value.width / 2, clientY: viewport.value.height / 2 }
    updateHover(); await nextTick(); updateMagnifier()
    // Paint the usable selector first; bounded edge analysis runs off the UI thread.
    probeRequest = requestAnimationFrame(() => { probeRequest = requestAnimationFrame(() => { try { findEdges() } catch { /* Native window candidates and manual selection remain usable. */ } }) })
  } catch (cause) { if (mounted) error.value = String(cause) }
})
onBeforeUnmount(() => {
  mounted = false; regionWorker?.terminate(); toolbarObserver?.disconnect()
  cancelAnimationFrame(moveRequest); cancelAnimationFrame(drawRequest); cancelAnimationFrame(probeRequest)
  window.removeEventListener('keydown', keydown); window.removeEventListener('keyup', keyup); window.removeEventListener('resize', resize)
  source = undefined; pendingMove = undefined
})
</script>

<template>
  <main ref="stage" class="capture-stage" aria-label="截图区域选择" :class="{ 'is-dragging': dragging }" @pointermove="move" @pointerdown="begin($event)" @pointerup="end" @pointercancel="abortPointer" @contextmenu.prevent="cancel">
    <img v-if="frame" class="capture-source" :src="frame.dataUrl" alt="当前冻结的屏幕画面" draggable="false" />
    <div v-if="!currentRect" class="capture-shade" />
    <div v-if="currentRect" class="capture-selection" :class="{ 'is-selected': selected, 'is-drawing': tool !== 'cursor' }" :style="selectionStyle" @pointerdown.stop="begin($event, selected ? 'move' : 'new')" @dblclick.stop="tool === 'cursor' && perform('copy')">
      <canvas v-show="selected && (annotations.length || draft)" ref="preview" class="capture-edit-preview" aria-label="截图标注预览" />
      <div class="capture-dimensions" :class="{ 'place-inside': currentRect.y / (frame?.height || 1) * viewport.height < 30 }"><strong>{{ currentRect.width }} × {{ currentRect.height }}</strong><span v-if="!selected">{{ hovered?.label }}</span></div>
      <template v-if="selected && !dragging && tool === 'cursor'">
        <button v-for="handle in handles" :key="handle" class="capture-handle" :class="`handle-${handle}`" :aria-label="`调整选区 ${handle}`" tabindex="-1" :disabled="busy" @pointerdown.stop="begin($event, handle)" />
      </template>
    </div>
    <aside v-if="!selected" class="capture-hint" @pointerdown.stop @pointerenter="hideMagnifier = true" @pointerleave="hideMagnifier = false">
      <span>{{ desktop ? '拖动或点击选区' : '原创演示 · 不读取屏幕' }}</span><span><kbd>Shift</kbd> 手动 <b>·</b> <kbd>Tab</kbd> 切换候选 <b>·</b> <kbd>Esc</kbd> 取消</span>
    </aside>
    <div v-show="showMagnifier" class="capture-magnifier" :style="magnifierStyle" aria-hidden="true"><canvas ref="magnifier" width="150" height="100" /><div><i :style="{ backgroundColor: pixelColor }" /><code>{{ pixelColor }}</code><small>{{ pointer.x }}, {{ pointer.y }}</small></div></div>
    <section v-if="selected && !dragging" ref="toolbar" class="capture-toolbar" :style="toolbarStyle" aria-label="截图工具条" @pointerdown.stop @pointermove.stop @pointerenter="hideMagnifier = true" @pointerleave="hideMagnifier = false">
      <div class="capture-tools" role="toolbar" aria-label="就地标注与截图操作">
        <button v-for="item in tools" :key="item.id" :title="`${item.label} (${item.key})`" :aria-label="item.label" :aria-pressed="tool === item.id" :class="{ active: tool === item.id }" :disabled="busy" @click="setTool(item.id)"><Icon :name="item.id === 'redact' ? 'rect' : item.id" :size="18"/><i v-if="item.id === 'redact'" class="redact-dot"/></button>
        <span class="separator"/>
        <input v-model="color" type="color" title="标注颜色" aria-label="标注颜色" :disabled="busy"/>
        <select v-model.number="lineWidth" title="线条粗细" aria-label="线条粗细" :disabled="busy"><option :value="2">细</option><option :value="3">中</option><option :value="6">粗</option></select>
        <button title="撤销（⌘/Ctrl Z）" aria-label="撤销" :disabled="busy || !canUndo" @click="undo"><Icon name="undo" :size="18"/></button>
        <button title="重做" aria-label="重做" :disabled="busy || !canRedo" @click="redo"><Icon name="redo" :size="18"/></button>
        <span class="separator"/>
        <button title="重新选择区域并清除本次标注" aria-label="重选区域" :disabled="busy" @click="reset"><Icon name="capture" :size="18"/></button>
        <button title="更多编辑：美化、贴纸与自动打码" aria-label="更多编辑" :disabled="busy || !validSelection" @click="perform('edit')"><Icon name="spark" :size="18"/></button>
        <button :title="desktop ? '贴到桌面' : '贴到桌面仅桌面版可用'" aria-label="贴到桌面" :disabled="busy || !desktop || !validSelection" @click="perform('pin')"><Icon name="pin" :size="18"/></button>
        <button title="保存 PNG（⌘/Ctrl S）" aria-label="保存截图" :disabled="busy || !validSelection" @click="perform('save')"><Icon name="save" :size="18"/></button>
        <button title="取消截图（Esc）" aria-label="取消截图" :disabled="busy" @click="cancel"><Icon name="close" :size="18"/></button>
        <button class="capture-confirm" title="复制图片并完成（Enter）" aria-label="复制截图" :disabled="busy || !validSelection" @click="perform('copy')"><Icon name="check" :size="18"/><span>{{ busy ? '处理中' : '复制' }}</span></button>
      </div>
      <div v-if="tool === 'text'" class="capture-text"><input v-model="text" maxlength="1000" placeholder="输入文字，再点击选区放置" aria-label="截图标注文字" :disabled="busy" /></div>
      <p v-if="error || status" class="capture-status" :class="{ failure: error }" :role="error ? 'alert' : 'status'">{{ error || status }}</p>
    </section>
    <div v-if="error && !selected" role="alert" class="capture-error" @pointerdown.stop><strong>截图暂不可用</strong><p>{{ error }}</p><button type="button" @click="cancel">取消并返回</button></div>
    <div v-if="!ready && !error" class="capture-loading" role="status">正在准备截图…</div>
  </main>
</template>

<style scoped>
.capture-stage{position:fixed;inset:0;background:#0b0e16;overflow:hidden;color:#f2f4fa;font-family:Inter,"PingFang SC","Microsoft YaHei",sans-serif;cursor:crosshair;user-select:none;touch-action:none}
.capture-stage button{font:inherit}.capture-source{position:absolute;inset:0;width:100%;height:100%;object-fit:fill;pointer-events:none}.capture-shade{position:absolute;inset:0;background:#06091470;pointer-events:none}.capture-selection{position:absolute;box-sizing:border-box;outline:1px solid #c8e7a0;box-shadow:0 0 0 9999px #05081375;z-index:1}.capture-selection.is-selected{cursor:move}.capture-selection.is-drawing{cursor:crosshair}.capture-edit-preview{position:absolute;left:0;top:0;width:100%;height:100%;pointer-events:none}.capture-dimensions{position:absolute;left:-1px;bottom:calc(100% + 5px);display:flex;gap:9px;padding:4px 7px;background:#151b25e8;border-radius:4px;white-space:nowrap;font-size:11px;pointer-events:none}.capture-dimensions.place-inside{bottom:auto;top:5px;left:5px}.capture-dimensions span{color:#adb7c7}.capture-dimensions strong{font-weight:550;font-variant-numeric:tabular-nums}.capture-handle{position:absolute;width:7px;height:7px;padding:0;border:1px solid #182316;background:#e1fda6;border-radius:1px}.handle-nw{top:-4px;left:-4px;cursor:nwse-resize}.handle-ne{top:-4px;right:-4px;cursor:nesw-resize}.handle-sw{bottom:-4px;left:-4px;cursor:nesw-resize}.handle-se{bottom:-4px;right:-4px;cursor:nwse-resize}.handle-n{top:-4px;left:calc(50% - 3px);cursor:ns-resize}.handle-s{bottom:-4px;left:calc(50% - 3px);cursor:ns-resize}.handle-w{left:-4px;top:calc(50% - 3px);cursor:ew-resize}.handle-e{right:-4px;top:calc(50% - 3px);cursor:ew-resize}.capture-hint{position:absolute;z-index:3;bottom:18px;left:50%;transform:translateX(-50%);display:flex;gap:16px;max-width:calc(100vw - 32px);padding:8px 12px;border:1px solid #ffffff1f;border-radius:6px;background:#151b25e8;cursor:default;font-size:11px;white-space:nowrap;color:#c3ccda}.capture-hint b{font-weight:400;margin:0 6px;color:#667386}kbd{font:inherit;color:#f1f5f9}.capture-magnifier{position:absolute;z-index:4;width:150px;padding:4px;border:1px solid #ffffff40;border-radius:6px;background:#101625f5;pointer-events:none}.capture-magnifier canvas{display:block;width:150px;height:100px}.capture-magnifier div{display:flex;align-items:center;justify-content:space-between;gap:4px;padding-top:5px}.capture-magnifier i{width:10px;height:10px;border:1px solid #ffffff25}.capture-magnifier code,.capture-magnifier small{font-size:10px;color:#c3ccda}.capture-toolbar{position:absolute;z-index:5;max-width:calc(100vw - 16px);box-sizing:border-box;padding:5px;background:#f9fafb;color:#263141;border:1px solid #d5dbe2;box-shadow:0 3px 12px #0003;border-radius:7px;cursor:default}.capture-tools{display:flex;flex-wrap:wrap;align-items:center;gap:2px}.capture-tools button{position:relative;display:inline-flex;align-items:center;justify-content:center;gap:4px;width:30px;height:30px;padding:0;border:0;border-radius:4px;background:transparent;color:inherit;cursor:pointer}.capture-tools button:hover{background:#e4e9f0}.capture-tools button.active{background:#e3efd6;color:#375824}.capture-tools button.capture-confirm{width:auto;min-width:57px;padding:0 8px;background:#dceacb;color:#2a441e;font-size:12px}.capture-tools button:disabled,.capture-tools input:disabled,.capture-tools select:disabled{opacity:.4;cursor:default}.capture-tools button:focus-visible,.capture-tools input:focus-visible,.capture-tools select:focus-visible{outline:2px solid #729452;outline-offset:1px}.separator{height:20px;width:1px;background:#d7dde5;margin:0 4px}.capture-tools input[type=color]{width:25px;height:25px;padding:2px;border:1px solid #d4dbe3;background:white;cursor:pointer;border-radius:4px}.capture-tools select{width:38px;height:26px;background:transparent;border:0;color:inherit;font-size:11px}.redact-dot{position:absolute;width:9px;height:8px;background:currentColor;pointer-events:none}.capture-text{padding:5px 2px 0}.capture-text input{box-sizing:border-box;width:100%;padding:7px 9px;border:1px solid #d0d8e2;border-radius:4px;font:inherit;font-size:12px;user-select:text;background:white;color:#263141}.capture-status{font-size:11px;line-height:1.5;white-space:normal;overflow-wrap:anywhere;margin:5px 4px 2px;max-width:600px;color:#375824}.capture-status.failure{color:#9d2f38}.capture-error{position:absolute;z-index:8;top:50%;left:50%;transform:translate(-50%,-50%);width:min(380px,calc(100vw - 48px));padding:25px;background:#151c2c;border:1px solid #ffb4ac66;border-radius:10px;cursor:default}.capture-error strong{color:#ffd0cb;font-size:15px}.capture-error p{font-size:13px;line-height:1.7;color:#abb6cb;overflow-wrap:anywhere}.capture-error button{background:#ffffff10;border:0;border-radius:5px;padding:9px;color:white;width:100%;cursor:pointer}.capture-loading{position:absolute;inset:0;display:grid;place-items:center;font-size:14px;color:#aebbd0;pointer-events:none}.is-dragging .capture-selection{cursor:crosshair}@media(max-width:600px){.capture-hint{gap:6px;font-size:10px}.capture-tools{gap:1px}.capture-tools button{width:28px}.separator{margin:0 2px}}
</style>
