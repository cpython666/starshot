<script setup lang="ts">
import { ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { SHARE_TEXT, createShareImage } from '../share';
const props=defineProps<{disabled?:boolean}>();
const picture=ref(''), message=ref(''), busy=ref(false);
const image=()=>picture.value || (picture.value=createShareImage());
async function action(kind: 'text'|'copy'|'save') {
  if(busy.value||props.disabled)return; busy.value=true;
  try {
    if(kind==='text') { if(isTauri()) await invoke('copy_text',{text:SHARE_TEXT}); else await navigator.clipboard.writeText(SHARE_TEXT); message.value='已复制分享文案。'; }
    else if(kind==='copy') {const dataUrl=image();if(isTauri())await invoke('copy_image',{dataUrl});else await navigator.clipboard.write([new ClipboardItem({'image/png':await(await fetch(dataUrl)).blob()})]);message.value='已复制产品介绍图。';}
    else if(isTauri()){const path=await invoke<string|null>('save_image',{dataUrl:image(),suggestedName:'星镜产品介绍.png'});message.value=path?`已保存到 ${path}`:'已取消保存。';}
    else {const a=document.createElement('a');a.href=image();a.download='星镜产品介绍.png';a.click();message.value='已发起产品介绍图下载。';}
  } catch(e) { message.value=String(e); } finally {busy.value=false;}
}
</script>
<template><section class="settings-section"><h3>分享星镜</h3><p class="field-note">本地截图、标注、贴图与图片美化。分享图使用原创示意画面。</p><fieldset :disabled="busy||disabled"><button class="button" @click="action('text')">复制介绍文案</button><button class="button" @click="image">预览产品图</button><button class="button" @click="action('copy')">复制产品图</button><button class="button" @click="action('save')">保存产品图</button></fieldset><img v-if="picture" :src="picture" class="share-preview" alt="星镜原创产品介绍图"/><p v-if="message" class="field-note" role="status">{{message}}</p></section></template>
