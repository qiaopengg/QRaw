import assert from 'node:assert/strict';
import test from 'node:test';
import { featureModule } from './componentHarness.mjs';

const { applySmartCullingEvent, useSmartCullingStore } = await featureModule('../useSmartCulling.ts');
const snapshot = (revision, state, taskId = 'first') => ({ revision, state, taskId, results: [] });

test('late analysis events cannot undo cancellation or a completed review transition', () => {
  const store = useSmartCullingStore.getState();
  store.setState({ snapshot: null });
  applySmartCullingEvent(snapshot(5, 'cancelling'));
  applySmartCullingEvent(snapshot(4, 'analyzing'));
  assert.equal(useSmartCullingStore.getState().snapshot.state, 'cancelling');
  applySmartCullingEvent(snapshot(6, 'readyForReview'));
  applySmartCullingEvent(snapshot(5, 'cancelling'));
  assert.equal(useSmartCullingStore.getState().snapshot.state, 'readyForReview');
  assert.equal(useSmartCullingStore.getState().screen, 'review');
});

test('an old task event cannot replace a newly inspected task, while newer events remain usable', () => {
  useSmartCullingStore.getState().setState({ snapshot: null, screen: 'setup' });
  applySmartCullingEvent(snapshot(20, 'configuring', 'second'));
  applySmartCullingEvent(snapshot(19, 'readyForReview', 'first'));
  assert.equal(useSmartCullingStore.getState().snapshot.taskId, 'second');
  assert.equal(useSmartCullingStore.getState().screen, 'setup');
  applySmartCullingEvent(snapshot(21, 'indexing', 'second'));
  assert.equal(useSmartCullingStore.getState().snapshot.state, 'indexing');
  assert.equal(useSmartCullingStore.getState().screen, 'analysis');
});
