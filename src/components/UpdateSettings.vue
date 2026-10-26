<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { createRecoveryPoint, initialUpdateStatus, listRecoveryPoints, readRecoveryPoint, restartAfterUpdate, updateDesktopAvailable, watchStarshotUpdates } from '../update-api';
import type { RecoveryPoint, UpdateStatus } from '../update-api';
import { downloadProgress, fileSize, updateInProgress, updateNeedsRestart, updatePhaseText, updateCanInstall } from '../update-state';
import { restoreWithBackup } from '../session';

const props = defineProps<{ disabled?: boolean; getSession: () => Promise<string>; restoreSession: (serialized: string) => Promise<void> }>();
const emit = defineEmits<{ busy: [value: boolean]; status: [value: UpdateStatus]; 'recovery-available': [point: RecoveryPoint | null] }>();
const status = ref(initialUpdateStatus()), statusLoaded = ref(false);
const points = ref<RecoveryPoint[]>([]), pointsLoaded = ref(false);
const operation = ref<'load' | 'check' | 'download' | 'install' | 'snapshot' | 'restore' | 'restart' | null>(null);
const error = ref(''), recoveryError = ref(''), notice = ref('');
const installVersion = ref<string | null>(null), restoreSelected = ref<RecoveryPoint | null>(null);
const confirmation = ref<HTMLElement>(), installDialog = ref<HTMLDialogElement>();
let disposed = false, promptedVersion: string | null = null;
const monitor = watchStarshotUpdates(value => { status.value = value; statusLoaded.value = true; emit('status', value); });
const needsRestart = computed(() => updateNeedsRestart(status.value));
const working = computed(() => operation.value !== null || updateInProgress(status.value));
const confirming = computed(() => !!installVersion.value || !!restoreSelected.value);
const blocked = computed(() => props.disabled || working.value || needsRestart.value);
const progress = computed(() => downloadProgress(status.value));
const canInstall = computed(() => updateCanInstall(status.value));
const canDownload = computed(() => !!status.value.available && !status.value.downloadReady && ['available', 'error'].includes(status.value.phase));
const upgradePoint = computed(() => points.value.find(point => point.reason === 'before-update' && point.compatible));
watch(() => confirming.value || needsRestart.value || updateInProgress(status.value) || ['install', 'snapshot', 'restore', 'restart'].includes(operation.value || ''), value => emit('busy', value), { immediate: true, flush: 'sync' });
// A repeated read of the same ready package must not reopen a dismissed prompt.
watch(status, value => {
  if (['checking', 'downloading'].includes(value.phase)) promptedVersion = null;
  const version = value.available?.version;
  if (value.phase === 'ready' && value.downloadReady && version && version !== promptedVersion && !confirming.value) {
    promptedVersion = version;
    installVersion.value = version;
  }
}, { flush: 'sync' });
watch(installVersion, async version => {
  if (!version) { installDialog.value?.close(); return; }
  await nextTick();
  if (!disposed && installVersion.value === version && installDialog.value) {
    if (!installDialog.value.open) installDialog.value.showModal();
    installDialog.value.focus();
  }
});

function message(reason: unknown) { return reason instanceof Error ? reason.message : String(reason); }
function date(value: number | string) {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? '日期未提供' : parsed.toLocaleString('zh-CN', { hour12: false });
}
function reasonLabel(reason: string) { return ({ manual: '手动保存', 'before-update': '升级前', 'before-restore': '恢复前' } as Record<string, string>)[reason] || '恢复点'; }
function addPoint(point: RecoveryPoint) { points.value = [point, ...points.value.filter(item => item.id !== point.id)]; pointsLoaded.value = true; }
async function perform(name: NonNullable<typeof operation.value>, action: () => Promise<void>) {
  const retryingRestart = name === 'restart' && needsRestart.value && operation.value !== 'restart';
  if (disposed || !updateDesktopAvailable || props.disabled || (working.value && !retryingRestart) || (needsRestart.value && name !== 'restart')) return;
  operation.value = name; error.value = ''; notice.value = '';
  try { await action(); } catch (reason) { if (!disposed) error.value = message(reason); }
  finally { if (!disposed && operation.value === name) operation.value = null; }
}
function refresh() {
  if (confirming.value) return;
  return perform('load', async () => {
    recoveryError.value = '';
    const results = await Promise.allSettled([monitor.read(), listRecoveryPoints()]);
    if (disposed) return;
    const [update, recovery] = results;
    if (update.status === 'rejected') error.value = `更新状态读取失败：${message(update.reason)}`;
    if (recovery.status === 'fulfilled') {
      points.value = recovery.value; pointsLoaded.value = true;
      emit('recovery-available', upgradePoint.value || null);
    } else recoveryError.value = `恢复点读取失败：${message(recovery.reason)}`;
  });
}
function check() { if (!confirming.value) return perform('check', () => monitor.check()); }
function download() {
  if (confirming.value || !canDownload.value) return;
  const expectedVersion = status.value.available!.version;
  return perform('download', async () => {
    try { await monitor.download(expectedVersion); }
    catch (reason) { try { await monitor.read(); } catch { /* Keep the last known state on transport failure. */ } throw reason; }
  });
}
function confirmInstall() {
  if (blocked.value || confirming.value || !canInstall.value) return;
  promptedVersion = status.value.available!.version;
  installVersion.value = promptedVersion;
}
function deferInstall() {
  if (blocked.value) return;
  installVersion.value = null;
  notice.value = '可以继续编辑，稍后再安装。关闭星镜后需重新下载更新。';
}
function installDialogKey(event: KeyboardEvent) {
  // Native <dialog> traps Tab and handles Escape; suppress editor/browser shortcuts.
  if ((event.metaKey || event.ctrlKey) && ['s', 'o', 'z'].includes(event.key.toLowerCase())) event.preventDefault();
}
async function confirmRestore(point: RecoveryPoint) {
  if (blocked.value || confirming.value || !point.compatible) return;
  restoreSelected.value = point;
  await nextTick(); if (!disposed) confirmation.value?.focus();
}
function cancelConfirmation() { if (!blocked.value) { installVersion.value = null; restoreSelected.value = null; } }
function install() {
  const approvedVersion = installVersion.value;
  if (!approvedVersion || !canInstall.value) return;
  return perform('install', async () => {
    // Capture only now: edits made after choosing “later” belong in the upgrade backup.
    const session = await props.getSession();
    try { await monitor.install(approvedVersion, session); }
    catch (reason) { try { await monitor.read(); } catch { /* Keep protection if native state cannot be confirmed. */ } throw reason; }
  });
}
function createSnapshot() {
  if (confirming.value) return;
  return perform('snapshot', async () => {
    const point = await createRecoveryPoint(await props.getSession());
    if (!disposed) { addPoint(point); notice.value = '当前会话已保存为本机恢复点。'; }
  });
}
function restore() {
  const selected = restoreSelected.value;
  if (!selected?.compatible) return;
  return perform('restore', async () => {
    await restoreWithBackup({
      getSession: props.getSession,
      backup: async session => { const point = await createRecoveryPoint(session); if (!disposed) addPoint(point); return point; },
      read: async () => (await readRecoveryPoint(selected.id)).session,
      apply: props.restoreSession,
    });
    if (!disposed) {
      restoreSelected.value = null; emit('recovery-available', null);
      notice.value = '已恢复原图、标注、裁剪、背景和界面外观。恢复前的会话已另存为恢复点；撤销历史从此处重新开始。';
    }
  });
}
function restart() {
  return perform('restart', async () => {
    notice.value = '正在重启星镜…';
    try { await restartAfterUpdate(); }
    catch (reason) { throw Error(`重启未完成，请再次点击「重试重启」。${message(reason)}`); }
  });
}
onMounted(() => { if (updateDesktopAvailable) void refresh(); });
onBeforeUnmount(() => { disposed = true; installDialog.value?.close(); monitor.dispose(); emit('busy', false); });
</script>

<template>
  <section class="update-settings" aria-labelledby="software-update-heading" :aria-busy="working">
    <div class="update-heading"><h3 id="software-update-heading">软件更新与会话恢复</h3><span>当前版本 {{ status.currentVersion }}</span></div>
    <div v-if="!updateDesktopAvailable" class="update-card">
      <strong>请在星镜桌面版使用更新与恢复</strong>
      <p>浏览器预览不会检查、下载或安装桌面更新，也不能读取或保存本机恢复点。</p>
    </div>
    <template v-else>
      <p v-if="!statusLoaded" class="update-hint" role="status">{{ operation === 'load' ? '正在读取本机状态…' : '尚未读取更新状态。' }}</p>
      <p v-else class="update-hint" :class="{ 'update-error': ['error', 'restart-required'].includes(status.phase) }" role="status">{{ updatePhaseText(status) }}</p>
      <p class="update-hint">手动检查、下载更新，不在启动时联网。下载完成后会提示「安装并重启」，确认后才备份当前会话并安装。</p>
      <div v-if="status.available" class="update-card">
        <div class="update-heading"><strong>新版本 {{ status.available.version }}</strong><span v-if="status.available.date">{{ date(status.available.date) }}</span></div>
        <p v-if="status.available.notes" class="update-notes">{{ status.available.notes }}</p><p v-else>此版本未提供更新说明。</p>
      </div>
      <div v-if="status.phase === 'downloading'" class="update-download" role="status">
        <progress aria-label="更新下载进度" :value="progress" max="100"></progress>
        <span>{{ fileSize(status.downloaded) }}<template v-if="status.total"> / {{ fileSize(status.total) }}</template><template v-if="progress !== undefined"> · {{ Math.floor(progress) }}%</template></span>
      </div>
      <div v-if="!needsRestart" class="update-actions">
        <button type="button" class="button" :disabled="blocked || confirming || (statusLoaded && !status.configured)" @click="statusLoaded ? check() : refresh()">{{ status.phase === 'checking' ? '正在检查…' : statusLoaded ? '检查更新' : '重新读取状态' }}</button>
        <button v-if="canDownload" type="button" class="button primary" :disabled="blocked || confirming" @click="download">更新</button>
        <button v-if="canInstall" type="button" class="button primary" :disabled="blocked || confirming" @click="confirmInstall">安装并重启</button>
      </div>
      <dialog ref="installDialog" class="install-dialog" data-update-install-dialog tabindex="-1" role="alertdialog" aria-labelledby="install-update-heading" aria-describedby="install-update-description" @cancel.prevent="deferInstall" @keydown.stop="installDialogKey">
        <h3 id="install-update-heading">{{ needsRestart ? '正在重启星镜…' : operation === 'install' ? '正在准备安装…' : '安装并重启' }}</h3>
        <p id="install-update-description">星镜 {{ installVersion }} 已下载并通过签名验证。确认后会保存当前最新编辑会话，安装更新并重新打开星镜。</p>
        <p class="update-hint">可以稍后安装，继续编辑图片；关闭星镜后需重新下载。重启后可从升级前恢复点继续，撤销历史不保留。</p>
        <p v-if="working || needsRestart" class="update-hint" role="status">{{ updatePhaseText(status) }}</p>
        <p v-if="error || status.error" class="update-error" role="alert">{{ error || status.error }}</p>
        <div v-if="!needsRestart" class="update-actions">
          <button class="button" :disabled="blocked" @click="deferInstall">稍后安装</button>
          <button class="button primary" :disabled="blocked || !canInstall" @click="install">{{ operation === 'install' ? '正在安装…' : '安装并重启' }}</button>
        </div>
        <div v-else class="update-actions"><span class="update-hint">若未自动重开，可以重试。</span><button class="button" :disabled="operation === 'restart' || disabled" @click="restart">重试重启</button></div>
      </dialog>
      <div v-if="needsRestart" class="update-confirmation" role="status">
        <strong>{{ status.phase === 'installed' ? '新版已安装，正在重启星镜…' : '需要重启星镜后继续' }}</strong>
        <p>当前编辑已暂停。升级前会话已保存在本机恢复点；重启后可在此恢复，不会静默加载图片。</p>
        <button class="button primary" :disabled="operation === 'restart' || disabled" @click="restart">{{ operation === 'restart' ? '正在重启…' : '重试重启' }}</button>
      </div>
      <div class="update-recovery">
        <div class="update-heading"><h4>本机会话恢复点</h4><button class="update-refresh" :disabled="blocked || confirming" @click="refresh">刷新本机状态</button></div>
        <p class="update-hint">保存当前会话中最多 6 张原图及各自标注、裁剪、背景、外观。不包含撤销历史。普通编辑只留在内存，手动备份或安装更新时才落盘。</p>
        <div v-if="upgradePoint" class="update-card"><strong>有一份升级前会话可恢复</strong><p>{{ date(upgradePoint.createdAt) }} · v{{ upgradePoint.appVersion }}。请在下方选择恢复；不会自动读取图片。</p></div>
        <button class="button" :disabled="blocked || confirming" @click="createSnapshot">{{ operation === 'snapshot' ? '正在保存…' : '备份当前会话' }}</button>
        <p v-if="pointsLoaded && !points.length" class="update-hint">还没有恢复点。可以手动备份，安装更新前也会保存。</p>
        <ul v-if="points.length" class="recovery-list" aria-label="本机恢复点">
          <li v-for="point in points" :key="point.id" class="update-card">
            <div class="recovery-row"><div><strong>{{ date(point.createdAt) }}</strong><span>{{ reasonLabel(point.reason) }} · v{{ point.appVersion }} · {{ fileSize(point.totalBytes) }}</span></div><button class="button" :disabled="blocked || confirming || !point.compatible" @click="confirmRestore(point)">恢复此会话</button></div>
            <p v-if="!point.compatible" class="update-error">此恢复点无法恢复，可能已损坏或由不兼容的新版创建。</p>
            <details><summary>查看保存位置</summary><code>{{ point.path }}</code></details>
          </li>
        </ul>
        <div v-if="restoreSelected && !needsRestart" ref="confirmation" class="update-confirmation" tabindex="-1" role="region" aria-label="确认恢复会话">
          <strong>恢复到 {{ date(restoreSelected.createdAt) }}？</strong>
          <p>当前编辑已暂停。会先备份当前会话，再完整检查这份恢复点中的原图和编辑状态，成功后替换当前会话与外观。之后新增的内容不会合并，撤销历史会重新开始；备份或检查失败则保留当前会话。</p>
          <div class="update-actions"><button class="button" :disabled="blocked" @click="cancelConfirmation">取消</button><button class="button primary" :disabled="blocked" @click="restore">备份当前会话并恢复</button></div>
        </div>
        <p v-if="recoveryError" class="update-error" role="alert">{{ recoveryError }}</p>
      </div>
      <p v-if="error" class="update-error" role="alert">{{ error }}</p><p v-else-if="notice" class="update-notice" role="status">{{ notice }}</p>
      <p class="update-hint">恢复点包含完整图片，以明文保存在本机，不上传。</p>
    </template>
  </section>
</template>

<style scoped>
.update-settings{border-top:1px solid var(--line);margin-top:12px;padding-top:20px;min-width:0}.update-heading,.recovery-row{display:flex;align-items:center;justify-content:space-between;gap:12px}.update-heading h3,.update-heading h4{font-size:12px;font-weight:500;margin:0}.update-heading>span,.recovery-row span{font-size:10px;color:var(--muted)}.update-hint,.update-card p,.update-confirmation p{font-size:11px;line-height:1.9;color:var(--muted);margin:10px 0}.update-card,.update-confirmation{padding:13px;border:1px solid var(--line);border-radius:8px;background:var(--input);margin-top:12px;min-width:0}.update-card strong,.update-confirmation>strong{font-size:12px;font-weight:500}.update-notes{white-space:pre-wrap;overflow-wrap:anywhere;max-height:200px;overflow-y:auto;user-select:text}.update-actions{display:flex;flex-wrap:wrap;gap:8px;margin-top:12px}.update-settings .button{font-size:11px;white-space:normal}.update-refresh{font-size:10px;color:var(--accent);padding:5px}.update-confirmation{border-color:var(--accent);background:var(--soft)}.update-confirmation>strong{color:var(--accent)}.update-recovery{margin-top:24px;padding-top:18px;border-top:1px solid var(--line)}.recovery-list{margin:12px 0;padding:0;list-style:none;max-height:350px;overflow:auto}.recovery-row>div{display:flex;flex-direction:column;gap:7px;min-width:0}.recovery-row .button{flex-shrink:0}.update-settings details{font-size:10px;line-height:1.8;color:var(--muted);margin-top:9px}.update-settings summary{cursor:pointer}.update-settings code{display:block;font:10px/1.8 ui-monospace,monospace;white-space:pre-wrap;overflow-wrap:anywhere;user-select:text;margin-top:6px}.update-error,.update-notice{font-size:11px;line-height:1.8;white-space:pre-wrap;overflow-wrap:anywhere;margin-top:12px}.update-settings .update-error{color:#d96c73}.update-notice{color:var(--accent)}.update-download{display:flex;flex-direction:column;gap:7px;margin-top:12px;font-size:10px;color:var(--muted)}.update-download progress{width:100%;height:7px;accent-color:var(--accent)}.update-confirmation:focus{outline:2px solid var(--accent);outline-offset:3px}@media(max-width:600px){.update-heading,.recovery-row{align-items:flex-start;flex-wrap:wrap}.update-actions .button{flex:1}}
.install-dialog{width:min(460px,calc(100vw - 48px));padding:26px;border:1px solid var(--line);border-radius:14px;background:var(--panel);color:var(--text);box-shadow:0 20px 80px #0005}.install-dialog::backdrop{background:#07091188;backdrop-filter:blur(5px)}.install-dialog h3{font-size:20px;font-weight:600;margin:0 0 16px}.install-dialog>p{font-size:13px;line-height:1.8;margin:12px 0}.install-dialog .update-actions{justify-content:flex-end;margin-top:22px}.install-dialog .button{font-size:13px;padding:10px 16px}.install-dialog:focus{outline:none}
</style>
