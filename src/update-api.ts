import packageInfo from '../package.json' with { type: 'json' };

export const appVersion = packageInfo.version;
export const updateDesktopAvailable = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
export type UpdatePhase = 'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'backing-up' | 'installing' | 'installed' | 'restart-required' | 'error';
export type UpdateStatus = {
  revision: number; currentVersion: string; configured: boolean; phase: UpdatePhase;
  available?: { version: string; date?: string | null; notes?: string | null } | null;
  downloaded: number; total?: number | null; error?: string | null; backupId?: string | null;
};
export type RecoveryPoint = { id: string; createdAt: number; appVersion: string; schemaVersion: number; reason: string; totalBytes: number; path: string; compatible: boolean };
export function initialUpdateStatus(): UpdateStatus { return { revision: 0, currentVersion: appVersion, configured: false, phase: 'idle', downloaded: 0 }; }
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!updateDesktopAvailable) throw Error('软件更新与恢复需要在星镜桌面版使用。');
  return (await import('@tauri-apps/api/core')).invoke<T>(command, args);
}
export type UpdateTransport = {
  invoke: (command: string, args?: Record<string, unknown>) => Promise<UpdateStatus>;
  listen: (accept: (state: UpdateStatus) => void) => Promise<() => void>;
};
const desktopTransport: UpdateTransport = {
  invoke,
  async listen(accept) {
    if (!updateDesktopAvailable) throw Error('软件更新与恢复需要在星镜桌面版使用。');
    return (await import('@tauri-apps/api/event')).listen<UpdateStatus>('starshot-update-status', event => accept(event.payload));
  },
};

// Subscribe before each first command; a late read cannot roll back newer event progress.
export function watchStarshotUpdates(onChange: (state: UpdateStatus) => void, transport = desktopTransport) {
  let disposed = false, revision = -1;
  let unsubscribe: (() => void) | undefined, subscribing: Promise<void> | undefined;
  function accept(state: UpdateStatus) {
    if (disposed || !Number.isSafeInteger(state.revision) || state.revision < revision) return;
    revision = state.revision; onChange(state);
  }
  async function subscribe() {
    if (disposed || unsubscribe) return;
    if (!subscribing) subscribing = transport.listen(accept).then(stop => { if (disposed) stop(); else unsubscribe = stop; }).finally(() => { subscribing = undefined; });
    await subscribing;
  }
  async function run(command: string, args?: Record<string, unknown>) {
    if (disposed) return;
    await subscribe();
    if (!disposed) accept(await transport.invoke(command, args));
  }
  return {
    read: () => run('get_update_status'), check: () => run('check_starshot_update'),
    install: (expectedVersion: string, session: string) => run('install_starshot_update', { expectedVersion, session }),
    dispose() { disposed = true; unsubscribe?.(); unsubscribe = undefined; },
  };
}
export function listRecoveryPoints(): Promise<RecoveryPoint[]> { return invoke('list_recovery_points'); }
export function createRecoveryPoint(session: string): Promise<RecoveryPoint> { return invoke('create_recovery_point', { session }); }
export function readRecoveryPoint(id: string): Promise<{ session: string; point: RecoveryPoint }> { return invoke('read_recovery_point', { id }); }
export function restartAfterUpdate(): Promise<void> { return invoke('restart_app'); }
