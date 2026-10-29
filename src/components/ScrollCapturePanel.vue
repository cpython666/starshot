<script setup lang="ts">
import Icon from './Icon.vue'

defineProps<{
  progress?: { width: number; height: number; frames: number; previewDataUrl: string; message: string }
  busy: boolean
  error: string
  status: string
}>()
defineEmits<{ append: []; finish: [action: 'copy' | 'save' | 'pin' | 'edit']; cancel: [] }>()
</script>

<template>
  <main class="scroll-capture" aria-label="滚动长截图" :aria-busy="busy">
    <header><strong><Icon name="scroll" :size="17" />滚动长截图</strong><button aria-label="取消长截图" title="取消（Esc）" :disabled="busy" @click="$emit('cancel')"><Icon name="close" :size="16" /></button></header>
    <p class="instructions">在原页面向下滚动，保留约半屏重叠。<br />等画面静止后，点击「采集下一屏」。</p>
    <section class="scroll-preview" aria-label="长截图拼接预览">
      <img v-if="progress" :src="progress.previewDataUrl" alt="当前已拼接的长截图" />
      <div v-if="progress"><strong>{{ progress.width }} × {{ progress.height }}</strong><span>已采集 {{ progress.frames }} 屏</span><small>采集时控制窗会暂时隐藏</small></div>
      <span v-else>正在准备长截图…</span>
    </section>
    <p class="scroll-status" :class="{ failure: error }" :role="error ? 'alert' : 'status'">{{ error || status || progress?.message || '只在点击采集时读取选区。' }}</p>
    <button class="append-screen" :disabled="busy || !progress" @click="$emit('append')"><Icon name="plus" :size="16" />{{ busy ? '正在处理…' : '采集下一屏' }}</button>
    <footer aria-label="长截图完成操作">
      <button :disabled="busy || !progress" @click="$emit('finish', 'edit')" title="在编辑器中标注、美化">更多编辑</button>
      <button :disabled="busy || !progress" @click="$emit('finish', 'pin')">贴到桌面</button>
      <button :disabled="busy || !progress" @click="$emit('finish', 'save')">保存</button>
      <button class="finish-copy" :disabled="busy || !progress" @click="$emit('finish', 'copy')">复制完成</button>
    </footer>
  </main>
</template>

<style scoped>
.scroll-capture{position:fixed;inset:0;box-sizing:border-box;display:flex;flex-direction:column;gap:8px;padding:12px 14px;background:#121a29;color:#e9eef8;font:12px/1.5 Inter,"PingFang SC","Microsoft YaHei",sans-serif;overflow:auto;user-select:none;border:1px solid #34435e}
header,header strong,footer{display:flex;align-items:center;gap:8px}header{justify-content:space-between}header strong{font-size:14px;font-weight:600;color:#d2eeb5}button{display:inline-flex;align-items:center;justify-content:center;gap:5px;min-height:29px;padding:4px 9px;border:1px solid #34435e;border-radius:5px;background:#202d42;color:inherit;font:inherit;cursor:pointer}header button{min-height:24px;padding:2px 4px;background:transparent;border:0}.instructions{margin:0;color:#b9c5d8;font-size:11px;line-height:1.6}.scroll-preview{display:flex;align-items:center;justify-content:center;gap:16px;height:76px;min-height:60px;border-radius:5px;background:#0b111d;overflow:hidden}.scroll-preview img{max-width:40%;height:100%;object-fit:contain;object-position:top}.scroll-preview div{display:flex;flex-direction:column;gap:2px}.scroll-preview strong{font-variant-numeric:tabular-nums;font-size:13px}.scroll-preview span{color:#b7c5dc}.scroll-preview small{color:#7186a7;font-size:10px}.scroll-status{margin:0;min-height:30px;max-height:44px;overflow:auto;font-size:11px;color:#a9c994;overflow-wrap:anywhere}.scroll-status.failure{color:#ffb0a6}.append-screen{width:100%;background:#2d4531;border-color:#506d44;color:#d8edc4;font-weight:600}footer{gap:6px}footer button{flex:1;padding-inline:4px;font-size:11px;white-space:nowrap}.finish-copy{background:#d4e9bd;color:#283b21;border-color:#d4e9bd}button:hover:not(:disabled){filter:brightness(1.15)}button:disabled{opacity:.45;cursor:default}button:focus-visible{outline:2px solid #c4e9a7;outline-offset:2px}
</style>
