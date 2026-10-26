<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
const emit = defineEmits<{ shortcuts: [value: {captureShortcut: string; pinShortcut: string}] }>();
const desktop = isTauri(), busy = ref(false), message = ref('');
const settings = ref({ captureShortcut: 'CommandOrControl+Shift+2', pinShortcut: 'CommandOrControl+Alt+3', startInTray: false, autostart: false });
async function refresh() {
  if (!desktop) return;
  try { settings.value = await invoke('get_desktop_settings'); emit('shortcuts', settings.value); }
  catch (e) { message.value = String(e); }
}
async function save() {
  busy.value = true;
  try { settings.value = await invoke('set_desktop_settings', { settings: settings.value }); emit('shortcuts', settings.value); message.value = '桌面设置已保存并生效。'; }
  catch (e) { const error=String(e);await refresh();message.value=error; }
  finally { busy.value = false; }
}
async function permissions() {
  try { const status = await invoke<{ captureAvailable: boolean; error?: string }>('get_app_status'); message.value = status.error || (status.captureAvailable ? '截图权限已可用。' : '请在系统设置中允许星镜录制屏幕，然后重新打开应用。'); }
  catch (e) { message.value = String(e); }
}
onMounted(refresh);
</script>
<template>
  <section class="settings-section">
    <h3>全局快捷键</h3>
    <fieldset :disabled="!desktop || busy">
      <label class="settings-input">开始截图<input v-model="settings.captureShortcut" aria-label="开始截图快捷键" maxlength="100" /></label>
      <label class="settings-input">贴最近原图<input v-model="settings.pinShortcut" aria-label="贴图快捷键" maxlength="100" /></label>
      <p class="field-note">CommandOrControl 在 Mac 表示 ⌘，在 Windows 表示 Ctrl；例如 CommandOrControl+Shift+2。冲突时保留原快捷键。</p>
      <label class="settings-input"><span>开机启动</span><input v-model="settings.autostart" type="checkbox" aria-label="开机启动"/></label><label class="settings-input"><span>启动时留在托盘</span><input v-model="settings.startInTray" type="checkbox" aria-label="启动时留在托盘"/></label><button class="button" @click="save">保存桌面设置</button>
      <button class="button" @click="permissions">重新检查截图权限</button>
    </fieldset>
    <p v-if="!desktop" class="field-note">全局快捷键与系统权限需要桌面版。</p>
    <p v-if="message" role="status" class="field-note">{{message}}</p>
  </section>
</template>
