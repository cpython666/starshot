import assert from 'node:assert/strict';
import test from 'node:test';
import { appVersion, initialUpdateStatus, watchStarshotUpdates, type UpdateStatus, type UpdateTransport } from '../src/update-api.ts';
import { downloadProgress, fileSize, updateInProgress, updateNeedsRestart } from '../src/update-state.ts';

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

test('install sends the explicitly approved version and frozen session after listener registration', async () => {
  const calls: unknown[] = [];
  const transport: UpdateTransport = {
    async listen() { calls.push('subscribe'); return () => {}; },
    async invoke(command, args) { calls.push([command, args]); return state(1, 'installed'); },
  };
  const monitor = watchStarshotUpdates(() => {}, transport);
  await monitor.install('0.2.0', '{"frozen":true}');
  assert.deepEqual(calls, ['subscribe', ['install_starshot_update', { expectedVersion: '0.2.0', session: '{"frozen":true}' }]]);
  monitor.dispose();
});

test('progress and write protection cover unknown totals, overshoot and pending restart', () => {
  assert.equal(initialUpdateStatus().currentVersion, appVersion);
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
