import assert from 'node:assert/strict';
import test, { beforeEach } from 'node:test';
import { featureModule } from './componentHarness.mjs';

const { runSmartCullingCommand, useSmartCullingStore } = await featureModule('../useSmartCulling.ts');

beforeEach(() => {
  useSmartCullingStore.getState().setState(useSmartCullingStore.getInitialState());
});

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const snapshot = (rating, revision = 1) => ({
  revision,
  taskId: 'portrait-task',
  state: 'readyForReview',
  mode: 'portrait',
  results: [{ resultId: 'photo', rating, source: 'manual', protected: true }],
});
const change = (rating) => ({ action: 'updateReview', changes: [{ resultId: 'photo', rating, colorLabel: null }] });

test('manual decisions reach the backend in user order and stay busy until both settle', async () => {
  const previousWindow = globalThis.window;
  const first = deferred();
  const second = deferred();
  const calls = [];
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (_command, { request }) => {
        calls.push(request);
        return calls.length === 1 ? first.promise : second.promise;
      },
    },
  };
  const older = runSmartCullingCommand(change(1), true);
  const newer = runSmartCullingCommand(change(5), true);
  try {
    await Promise.resolve();
    assert.equal(calls.length, 1, 'a later edit must not overtake an unfinished earlier edit');
    first.resolve(snapshot(1));
    await older;
    assert.equal(useSmartCullingStore.getState().busy, true, 'another edit is still pending');
    second.resolve(snapshot(5, 2));
    await newer;
    assert.deepEqual(
      calls.map((request) => request.changes[0].rating),
      [1, 5],
    );
    assert.equal(useSmartCullingStore.getState().snapshot.results[0].rating, 5);
    assert.equal(useSmartCullingStore.getState().busy, false);
  } finally {
    first.resolve(snapshot(1));
    second.resolve(snapshot(5, 2));
    await Promise.allSettled([older, newer]);
    globalThis.window = previousWindow;
  }
});

test('a failed command does not drop the next manual decision or leave the UI busy', async () => {
  const previousWindow = globalThis.window;
  const first = deferred();
  let calls = 0;
  globalThis.window = {
    __TAURI_INTERNALS__: { invoke: () => (++calls === 1 ? first.promise : Promise.resolve(snapshot(4))) },
  };
  const failed = runSmartCullingCommand(change(2), true);
  const observedFailure = assert.rejects(failed);
  const next = runSmartCullingCommand(change(4), true);
  try {
    first.reject({ code: 'updateReview_failed', detail: 'fixture failure' });
    await observedFailure;
    await next;
    assert.equal(calls, 2);
    assert.equal(useSmartCullingStore.getState().snapshot.results[0].rating, 4);
    assert.equal(useSmartCullingStore.getState().error, null);
    assert.equal(useSmartCullingStore.getState().busy, false);
  } finally {
    await Promise.allSettled([failed, next]);
    globalThis.window = previousWindow;
  }
});

test('cancelling an already running task is not blocked by a library write', async () => {
  const previousWindow = globalThis.window;
  const locking = deferred();
  const calls = [];
  useSmartCullingStore.getState().setState({ snapshot: { ...snapshot(0), state: 'analyzing' } });
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (_command, { request }) => {
        calls.push(request.action);
        return request.action === 'setLock'
          ? locking.promise
          : Promise.resolve({ ...snapshot(0, 2), state: 'cancelling' });
      },
    },
  };
  const write = runSmartCullingCommand({ action: 'setLock', paths: ['/photo.jpg'], locked: true }, true);
  const cancel = runSmartCullingCommand({ action: 'cancel' });
  try {
    await Promise.resolve();
    assert.ok(calls.includes('cancel'), 'cancellation must not wait for the library transaction');
    await cancel;
    assert.equal(useSmartCullingStore.getState().busy, true);
  } finally {
    locking.resolve({ ...snapshot(0, 3), state: 'cancelling' });
    await Promise.allSettled([write, cancel]);
    globalThis.window = previousWindow;
  }
});

test('cancel requested before start completes still waits for a task to exist', async () => {
  const previousWindow = globalThis.window;
  const starting = deferred();
  const calls = [];
  useSmartCullingStore.getState().setState({ snapshot: { ...snapshot(0), state: 'configuring' } });
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (_command, { request }) => {
        calls.push(request.action);
        return request.action === 'start'
          ? starting.promise
          : Promise.resolve({ ...snapshot(0, 3), state: 'cancelling' });
      },
    },
  };
  const start = runSmartCullingCommand({ action: 'start', rootPath: '/photos', mode: 'portrait', keyPeople: [] });
  const cancel = runSmartCullingCommand({ action: 'cancel' });
  try {
    await Promise.resolve();
    assert.deepEqual(calls, ['start']);
    starting.resolve({ ...snapshot(0, 2), state: 'indexing' });
    await Promise.all([start, cancel]);
    assert.deepEqual(calls, ['start', 'cancel']);
  } finally {
    starting.resolve({ ...snapshot(0, 2), state: 'indexing' });
    await Promise.allSettled([start, cancel]);
    globalThis.window = previousWindow;
  }
});

test('a late start response cannot undo cancellation after its indexing event was received', async () => {
  const previousWindow = globalThis.window;
  const starting = deferred();
  const calls = [];
  const indexing = { ...snapshot(0, 11), state: 'indexing' };
  const cancelling = { ...snapshot(0, 12), state: 'cancelling' };
  useSmartCullingStore.getState().setState({ snapshot: { ...snapshot(0, 10), state: 'configuring' } });
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (_command, { request }) => {
        calls.push(request.action);
        return request.action === 'start' ? starting.promise : Promise.resolve(cancelling);
      },
    },
  };
  const start = runSmartCullingCommand({ action: 'start', rootPath: '/photos', mode: 'portrait', keyPeople: [] });
  let cancel;
  try {
    await Promise.resolve();
    assert.deepEqual(calls, ['start']);
    // The backend emits this event before returning the start invocation.
    useSmartCullingStore.getState().setState({ snapshot: indexing, screen: 'analysis' });
    cancel = runSmartCullingCommand({ action: 'cancel' });
    await cancel;
    assert.equal(useSmartCullingStore.getState().snapshot.state, 'cancelling');
    starting.resolve(indexing);
    await start;
    const current = useSmartCullingStore.getState();
    assert.deepEqual(
      { state: current.snapshot.state, revision: current.snapshot.revision },
      { state: 'cancelling', revision: 12 },
      'the old start response must not overwrite the newer cancellation',
    );
    assert.equal(current.busy, false);
  } finally {
    starting.resolve(indexing);
    await Promise.allSettled([start, cancel]);
    globalThis.window = previousWindow;
  }
});

test('a later abandon waits for the bypassed cancellation to settle', async () => {
  const previousWindow = globalThis.window;
  const cancelling = deferred();
  const calls = [];
  const cancelledSnapshot = { ...snapshot(0, 21), state: 'cancelling' };
  const abandonedSnapshot = { ...snapshot(0, 23), state: 'idle', taskId: null, results: [] };
  useSmartCullingStore.getState().setState({ snapshot: { ...snapshot(0, 20), state: 'analyzing' } });
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (_command, { request }) => {
        calls.push(request.action);
        return request.action === 'cancel' ? cancelling.promise : Promise.resolve(abandonedSnapshot);
      },
    },
  };
  const cancel = runSmartCullingCommand({ action: 'cancel' });
  // Analysis can finish and emit readyForReview while the cancel response is in flight.
  useSmartCullingStore.getState().setState({ snapshot: snapshot(0, 22), screen: 'review' });
  const abandon = runSmartCullingCommand({ action: 'abandon' });
  try {
    await Promise.resolve();
    assert.deepEqual(calls, ['cancel'], 'a later task mutation must not overtake the pending cancellation');
    assert.equal(useSmartCullingStore.getState().busy, true);
    cancelling.resolve(cancelledSnapshot);
    await Promise.all([cancel, abandon]);
    assert.deepEqual(calls, ['cancel', 'abandon']);
    const current = useSmartCullingStore.getState();
    assert.equal(current.snapshot.state, 'idle');
    assert.equal(current.snapshot.taskId, null);
    assert.equal(current.busy, false);
  } finally {
    cancelling.resolve(cancelledSnapshot);
    await Promise.allSettled([cancel, abandon]);
    globalThis.window = previousWindow;
  }
});
