import assert from 'node:assert/strict';
import test from 'node:test';
import { featureModule } from './componentHarness.mjs';

const { useSmartCullingStore, runSmartCullingCommand } = await featureModule('../useSmartCulling.ts');

test('a late folder preflight cannot overwrite the shooting mode selected by the user', async () => {
  let finish;
  const response = new Promise((resolve) => {
    finish = resolve;
  });
  const previousWindow = globalThis.window;
  globalThis.window = { __TAURI_INTERNALS__: { invoke: () => response } };
  try {
    useSmartCullingStore.getState().setState({ mode: 'auto', screen: 'setup', snapshot: null });
    const preflight = runSmartCullingCommand({ action: 'inspect', rootPath: '/test-fixtures' });
    assert.equal(useSmartCullingStore.getState().busy, true);
    useSmartCullingStore.getState().setState({ mode: 'portrait' });
    finish({ state: 'configuring', mode: 'auto', results: [] });
    await preflight;
    assert.equal(useSmartCullingStore.getState().mode, 'portrait');
    assert.equal(useSmartCullingStore.getState().busy, false);
  } finally {
    globalThis.window = previousWindow;
  }
});
