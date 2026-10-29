<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
const emit = defineEmits<{ shortcuts: [value: {captureShortcut: string; pinShortcut: string}] }>();
const desktop = isTauri(), busy = ref(false), message = ref('');
const detection = ref<{ platform: string; elementDetection?: boolean }>();
const settings = ref({ captureShortcut: 'CommandOrControl+Shift+2', pinShortcut: 'CommandOrControl+Alt+3', startInTray: false, autostart: false, pinHalo: true });
let unlisten: UnlistenFn | undefined, mounted = true, haloRevision = 0;
function applySettings(loaded: typeof settings.value, revision: number) {
  settings.value = { ...loaded, pinHalo: revision === haloRevision ? loaded.pinHalo ?? true : settings.value.pinHalo };
  emit('shortcuts', { captureShortcut: settings.value.captureShortcut, pinShortcut: settings.value.pinShortcut });
}
async function refresh() {
  if (!desktop) return;
  const revision = haloRevision;
  try { applySettings(await invoke('get_desktop_settings'), revision); detection.value = await invoke('get_app_status'); }
  catch (e) { message.value = String(e); }
}
async function save() {
  busy.value = true;
  const revision = haloRevision;
  try { applySettings(await invoke('set_desktop_settings', { settings: settings.value }), revision); message.value = '桌面设置已保存并生效。'; }
  catch (e) { const error=String(e);await refresh();message.value=error; }
  finally { busy.value = false; }
}
async function permissions() {
  try { const status = await invoke<{ platform: string; captureAvailable: boolean; elementDetection?: boolean; error?: string }>('get_app_status'); detection.value = status; message.value = status.error || (status.captureAvailable ? '截图权限已可用。' : '请在系统设置 → 隐私与安全性 → 屏幕与系统音频录制中允许 Starshot（星镜），然后重新打开应用。'); }
  catch (e) { message.value = String(e); }
}
onMounted(async () => {
  if (desktop) {
    try {
      const off = await listen<boolean>('pin-halo', event => { haloRevision++; settings.value.pinHalo = event.payload; });
      if (!mounted) { off(); return; }
      unlisten = off;
    } catch (e) { message.value = String(e); }
  }
  if (mounted) await refresh();
});
onBeforeUnmount(() => { mounted = false; unlisten?.(); });
</script>
<template>
  <section class="settings-section">
    <h3>全局快捷键</h3>
    <fieldset :disabled="!desktop || busy">
      <label class="settings-input">开始截图<input v-model="settings.captureShortcut" aria-label="开始截图快捷键" maxlength="100" /></label>
      <label class="settings-input">贴最近原图<input v-model="settings.pinShortcut" aria-label="贴图快捷键" maxlength="100" /></label>
      <p class="field-note">CommandOrControl 在 Mac 表示 ⌘，在 Windows 表示 Ctrl；例如 CommandOrControl+Shift+2。冲突时保留原快捷键。</p>
      <label class="settings-input"><span>贴图焦点光圈</span><input v-model="settings.pinHalo" type="checkbox" aria-label="贴图焦点光圈"/></label>
      <p class="field-note">默认开启：活跃贴图显示蓝色光圈，其他贴图显示灰色阴影。右键贴图也可开关，选择会保存在本机。</p>
      <label class="settings-input"><span>开机启动</span><input v-model="settings.autostart" type="checkbox" aria-label="开机启动"/></label><label class="settings-input"><span>启动时留在托盘</span><input v-model="settings.startInTray" type="checkbox" aria-label="启动时留在托盘"/></label><button class="button" @click="save">保存桌面设置</button>
      <button class="button" @click="permissions">重新检查截图权限</button>
    </fieldset>
    <p v-if="detection?.platform === 'macos'" class="field-note">{{ detection.elementDetection ? '精准控件预选已可用：截图时读取鼠标下控件的位置和尺寸，不读取文字。' : '更精确的控件预选：可在系统设置 → 隐私与安全性 → 辅助功能中允许 Starshot（星镜）。未开启时仍可预选窗口、画面边缘或手动框选。' }}</p>
    <p v-if="!desktop" class="field-note">全局快捷键与系统权限需要桌面版。</p>
    <p v-if="message" role="status" class="field-note">{{message}}</p>
  </section>
</template>
