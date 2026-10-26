<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { createRecoveryPoint, initialUpdateStatus, listRecoveryPoints, readRecoveryPoint, restartAfterUpdate, updateDesktopAvailable, watchStarshotUpdates } from '../update-api';
import type { RecoveryPoint, UpdateStatus } from '../update-api';
import { downloadProgress, fileSize, updateInProgress, updateNeedsRestart, updatePhaseText } from '../update-state';
import { restoreWithBackup } from '../session';

const props = defineProps<{ disabled?: boolean; getSession: () => Promise<string>; restoreSession: (serialized: string) => Promise<void> }>();
const emit = defineEmits<{ busy: [value: boolean]; status: [value: UpdateStatus]; 'recovery-available': [point: RecoveryPoint | null] }>();
const status = ref(initialUpdateStatus()), statusLoaded = ref(false);
const points = ref<RecoveryPoint[]>([]), pointsLoaded = ref(false);
const operation = ref<'load' | 'check' | 'install' | 'snapshot' | 'restore' | 'restart' | null>(null);
const error = ref(''), recoveryError = ref(''), notice = ref('');
const installVersion = ref<string | null>(null), restoreSelected = ref<RecoveryPoint | null>(null);
const confirmation = ref<HTMLElement>();
let disposed = false;
const monitor = watchStarshotUpdates(value => { status.value = value; statusLoaded.value = true; emit('status', value); });
const needsRestart = computed(() => updateNeedsRestart(status.value));
const working = computed(() => operation.value !== null || updateInProgress(status.value));
const confirming = computed(() => !!installVersion.value || !!restoreSelected.value);
const blocked = computed(() => props.disabled || working.value || needsRestart.value);
const progress = computed(() => downloadProgress(status.value));
const canInstall = computed(() => !!status.value.available && ['available', 'error'].includes(status.value.phase));
const upgradePoint = computed(() => points.value.find(point => point.reason === 'before-update' && point.compatible));
watch(() => confirming.value || needsRestart.value || updateInProgress(status.value) || ['install', 'snapshot', 'restore', 'restart'].includes(operation.value || ''), value => emit('busy', value), { immediate: true, flush: 'sync' });

function message(reason: unknown) { return reason instanceof Error ? reason.message : String(reason); }
function date(value: number | string) {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? '日期未提供' : parsed.toLocaleString('zh-CN', { hour12: false });
}
function reasonLabel(reason: string) { return ({ manual: '手动保存', 'before-update': '升级前', 'before-restore': '恢复前' } as Record<string, string>)[reason] || '恢复点'; }
function addPoint(point: RecoveryPoint) { points.value = [point, ...points.value.filter(item => item.id !== point.id)]; pointsLoaded.value = true; }
async function perform(name: NonNullable<typeof operation.value>, action: () => Promise<void>) {
  if (disposed || !updateDesktopAvailable || props.disabled || working.value || (needsRestart.value && name !== 'restart')) return;
  operation.value = name; error.value = ''; notice.value = '';
  try { await action(); } catch (reason) { if (!disposed) error.value = message(reason); }
  finally { if (!disposed) operation.value = null; }
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
async function confirmInstall() {
  if (blocked.value || confirming.value || !canInstall.value) return;
  installVersion.value = status.value.available!.version;
  await nextTick(); if (!disposed) confirmation.value?.focus();
}
async function confirmRestore(point: RecoveryPoint) {
  if (blocked.value || confirming.value || !point.compatible) return;
  restoreSelected.value = point;
  await nextTick(); if (!disposed) confirmation.value?.focus();
}
function cancelConfirmation() { if (!blocked.value) { installVersion.value = null; restoreSelected.value = null; } }
function install() {
  const approvedVersion = installVersion.value;
  if (!approvedVersion) return;
  return perform('install', async () => {
    // Capture the frozen editor at confirmation, keeping the approved version fixed across awaits.
    const session = await props.getSession();
    installVersion.value = null;
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
    catch (reason) { throw Error(`自动重启未完成，请再次点击「重启星镜」。${message(reason)}`); }
  });
}
onMounted(() => { if (updateDesktopAvailable) void refresh(); });
onBeforeUnmount(() => { disposed = true; monitor.dispose(); emit('busy', false); });
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
      <p class="update-hint">仅点击「检查更新」才联网。更新包验证后，先保存当前会话，再安装。安装期间编辑暂停。</p>
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
        <button v-if="canInstall" type="button" class="button primary" :disabled="blocked || confirming" @click="confirmInstall">下载并安装</button>
      </div>
      <div v-if="installVersion && !needsRestart" ref="confirmation" class="update-confirmation" tabindex="-1" role="region" aria-label="确认安装更新">
        <strong>安装 {{ installVersion }}？</strong>
        <p>当前编辑已暂停。确认后下载并验证更新、备份当前会话，再安装。Windows 可能自动退出，macOS 需重启生效。重启会清空内存，重新打开后可从升级前恢复点继续；不会自动加载图片。恢复点不含撤销历史。</p>
        <div class="update-actions"><button class="button" :disabled="blocked" @click="cancelConfirmation">暂不安装</button><button class="button primary" :disabled="blocked" @click="install">确认下载、备份并安装 {{ installVersion }}</button></div>
      </div>
      <div v-if="needsRestart" class="update-confirmation" role="status">
        <strong>{{ status.phase === 'installed' ? '新版已安装，重启后继续' : '需要重启星镜后继续' }}</strong>
        <p>当前编辑已暂停。升级前会话已保存在本机恢复点；重启后可在此恢复，不会静默加载图片。</p>
        <button class="button primary" :disabled="operation !== null || disabled" @click="restart">{{ operation === 'restart' ? '正在重启…' : '重启星镜' }}</button>
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
</style>
