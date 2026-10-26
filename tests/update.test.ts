import assert from 'node:assert/strict';
import test from 'node:test';
import { appVersion, initialUpdateStatus, watchStarshotUpdates, type UpdateStatus, type UpdateTransport } from '../src/update-api.ts';
import { downloadProgress, fileSize, updateCanInstall, updateInProgress, updateIsReady, updateNeedsRestart } from '../src/update-state.ts';

const state = (revision: number, phase: UpdateStatus['phase'] = 'idle'): UpdateStatus => ({ ...initialUpdateStatus(), revision, phase });

test('update monitor subscribes once, ignores stale command results, and disposes late subscriptions', async () => {
  let accept!: (state: UpdateStatus) => void, resolveRead!: (state: UpdateStatus) => void;
  let subscribeCount = 0, stopCount = 0;
  const received: UpdateStatus[] = [];
  const monitor = watchStarshotUpdates(value => received.push(value), {
    async listen(callback) { subscribeCount++; accept = callback; return () => { stopCount++; }; },
    async invoke() { return new Promise(resolve => { resolveRead = resolve; }); },
  });
  const read = monitor.read(); await new Promise(resolve => setImmediate(resolve));
  accept(state(4, 'downloading')); resolveRead(state(1)); await read;
  assert.deepEqual(received.map(s => s.revision), [4]);
  assert.equal(subscribeCount, 1);
  monitor.dispose(); accept(state(5, 'installed')); await monitor.read();
  assert.equal(stopCount, 1); assert.equal(received.length, 1);
  let finishListen!: (stop: () => void) => void;
  const late = watchStarshotUpdates(() => assert.fail('disposed monitor emitted'), {
    listen: async () => new Promise(resolve => { finishListen = resolve; }),
    invoke: async () => { assert.fail('disposed monitor invoked'); },
  });
  const pending = late.read(); late.dispose(); finishListen(() => { stopCount++; }); await pending;
  assert.equal(stopCount, 2);
});

test('download sends only the selected version; installation is a separate explicit command with the latest session', async () => {
  const calls: unknown[] = [];
  const transport: UpdateTransport = {
    async listen() { calls.push('subscribe'); return () => {}; },
    async invoke(command, args) { calls.push([command, args]); return state(calls.length, command === 'download_starshot_update' ? 'ready' : 'installed'); },
  };
  const monitor = watchStarshotUpdates(() => {}, transport);
  await monitor.download('3.0.1');
  assert.deepEqual(calls, ['subscribe', ['download_starshot_update', { expectedVersion: '3.0.1' }]]);
  await monitor.confirm('3.0.1', false);
  assert.deepEqual(calls.at(-1), ['set_update_confirmation', { expectedVersion: '3.0.1', pending: false }]);
  await monitor.confirm('3.0.1', true);
  assert.deepEqual(calls.at(-1), ['set_update_confirmation', { expectedVersion: '3.0.1', pending: true }]);
  await monitor.install('3.0.1', '{"latestEdits":true}');
  assert.deepEqual(calls.at(-1), ['install_starshot_update', { expectedVersion: '3.0.1', session: '{"latestEdits":true}' }]);
  assert.equal(calls.filter(call => call === 'subscribe').length, 1);
  monitor.dispose();
});

test('only a verified cached candidate permits installation, including retries after a backup failure', () => {
  const ready = { ...state(1, 'ready'), available: { version: '3.0.1' }, downloadReady: true, confirmationPending: true };
  assert.equal(updateCanInstall(ready), true);
  assert.equal(updateInProgress(ready), false);
  assert.equal(updateNeedsRestart(ready), false);
  assert.equal(updateCanInstall({ ...ready, confirmationPending: false }), false);
  assert.equal(updateIsReady({ ...ready, confirmationPending: false }), true);
  assert.equal(updateCanInstall({ ...ready, phase: 'error' }), true);
  assert.equal(updateCanInstall({ ...ready, downloadReady: false }), false);
  assert.equal(updateCanInstall({ ...ready, available: null }), false);
  for (const phase of ['available', 'checking', 'downloading', 'backing-up', 'installing', 'installed', 'restart-required'] as const) {
    assert.equal(updateCanInstall({ ...ready, phase }), false);
  }
  assert.equal(updateCanInstall({ ...ready, phase: 'error', downloadReady: false }), false);
});

test('progress and write protection cover unknown totals, overshoot and pending restart', () => {
  assert.equal(initialUpdateStatus().currentVersion, appVersion);
  assert.equal(initialUpdateStatus().confirmationPending, false);
  assert.equal(downloadProgress({ ...state(1), downloaded: 50 }), undefined);
  assert.equal(downloadProgress({ ...state(1), downloaded: 50, total: 100 }), 50);
  assert.equal(downloadProgress({ ...state(1), downloaded: 150, total: 100 }), 100);
  assert.equal(downloadProgress({ ...state(1), downloaded: -10, total: 100 }), 0);
  assert.equal(downloadProgress({ ...state(1), total: Infinity }), undefined);
  for (const phase of ['checking', 'downloading', 'backing-up', 'installing'] as const) assert.equal(updateInProgress(state(1, phase)), true);
  for (const phase of ['installed', 'restart-required'] as const) assert.equal(updateNeedsRestart(state(1, phase)), true);
  assert.equal(updateInProgress(state(1, 'error')), false);
  assert.equal(fileSize(NaN), '大小未知'); assert.equal(fileSize(1024), '1 KiB');
});
