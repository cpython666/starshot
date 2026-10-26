<script setup lang="ts">
import { PRIVACY_KINDS, type PrivacyCandidate } from '../privacy';
defineProps<{ candidates: PrivacyCandidate[]; busy: boolean; desktop: boolean; message: string }>();
const candidateLabel = (candidate: PrivacyCandidate) => candidate.kinds.map(kind => PRIVACY_KINDS[kind]).join('、');
defineEmits<{ scan: []; apply: []; clear: []; toggle: [id: string] }>();
</script>
<template>
  <section class="control-section privacy-panel">
    <h3>敏感信息打码</h3>
    <p class="field-note">识别当前裁剪图片、标注和贴纸中的手机号、邮箱、证件、银行卡及常见凭据，不扫描装饰背景；勾选候选区域后实心遮盖。导出前请复核漏识别处。</p>
    <button class="button full" :disabled="busy || !desktop" @click="$emit('scan')">{{busy ? '正在本机识别…' : '识别敏感信息'}}</button>
    <p v-if="!desktop" class="field-note">文字识别需要桌面版；浏览器可手动画实心遮盖。</p>
    <p class="field-note">Mac 支持中文与英文；Windows 离线模型主要支持拉丁文字和数字。</p>
    <p v-if="message" class="field-note" role="status">{{message}}</p>
    <div v-if="candidates.length" class="privacy-candidates" aria-label="敏感信息候选">
      <label v-for="(candidate, i) in candidates" :key="candidate.id"><input type="checkbox" :checked="candidate.selected" :disabled="busy" @change="$emit('toggle',candidate.id)"/>第 {{i+1}} 处 · {{candidateLabel(candidate)}}</label>
      <button class="button primary full" :disabled="busy || !candidates.some(c=>c.selected)" @click="$emit('apply')">遮盖选中项（可撤销）</button>
      <button class="button full" :disabled="busy" @click="$emit('clear')">取消候选框</button>
    </div>
  </section>
</template>
