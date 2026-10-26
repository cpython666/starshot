import type { UpdateStatus } from './update-api.ts';

export function updateInProgress(state: UpdateStatus): boolean { return ['checking', 'downloading', 'backing-up', 'installing'].includes(state.phase); }
export function updateNeedsRestart(state: UpdateStatus): boolean { return ['installed', 'restart-required'].includes(state.phase); }
export function downloadProgress(state: UpdateStatus): number | undefined {
  if (!state.total || !Number.isFinite(state.total) || state.total <= 0) return undefined;
  return Math.min(100, Math.max(0, Number.isFinite(state.downloaded) ? state.downloaded / state.total * 100 : 0));
}
export function updatePhaseText(state: UpdateStatus): string {
  switch (state.phase) {
    case 'checking': return '正在检查新版本…';
    case 'up-to-date': return '已检查，当前没有更新的版本。';
    case 'available': return state.available ? `发现新版本 ${state.available.version}。` : '有新版本可用。';
    case 'downloading': return '正在下载更新…';
    case 'backing-up': return '正在保存升级前的会话恢复点…';
    case 'installing': return '正在安装更新，请稍候…';
    case 'installed': return '新版已安装，重启星镜后生效。';
    case 'restart-required': return state.error || '需要重启星镜后继续。';
    case 'error': return state.error || '更新未能完成，请重试。';
    default: return state.configured ? '按需检查更新，不在启动时自动联网。' : '此构建尚未配置更新服务。';
  }
}
export function fileSize(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '大小未知';
  if (value < 1024) return `${Math.round(value)} B`;
  const units = ['KiB', 'MiB', 'GiB', 'TiB'];
  let amount = value / 1024, unit = 0;
  while (amount >= 1024 && unit < units.length - 1) { amount /= 1024; unit++; }
  return `${amount.toLocaleString('zh-CN', { maximumFractionDigits: 1 })} ${units[unit]}`;
}
