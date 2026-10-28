import assert from 'node:assert/strict';
import test from 'node:test';
import { featureModule, renderComponent } from './componentHarness.mjs';

const { useSmartCullingStore, runSmartCullingCommand } = await featureModule('../useSmartCulling.ts');
const { canInspectSmartCullingFolder, isSmartCullingSource } = await featureModule('../sourceEligibility.ts');
const { default: SmartCullingLockControl } = await featureModule('../SmartCullingLockControl.tsx');
const { default: SmartCullingEntry } = await featureModule('../SmartCullingEntry.tsx');

const localImage = {
  path: '/photos/photo.raw',
  rating: 0,
  tags: [],
  is_virtual_copy: false,
  is_cloud_placeholder: false,
};

test('folder inspection accepts real folders and rejects album display paths', () => {
  assert.equal(canInspectSmartCullingFolder('/photos', null), true);
  assert.equal(canInspectSmartCullingFolder('C:\\Photos', null), true);
  assert.equal(canInspectSmartCullingFolder(null, null), false);
  assert.equal(canInspectSmartCullingFolder('', null), false);
  for (const albumId of ['local-album', 'immich:album-id', 'immich:?{"takenFrom":"2026-01-01"}']) {
    assert.equal(canInspectSmartCullingFolder('Album: Vacation', albumId), false);
  }
});

test('smart-culling writes exclude cloud placeholders and Immich assets while retaining local album images', () => {
  assert.equal(isSmartCullingSource(localImage, null), true);
  assert.equal(isSmartCullingSource(localImage, 'local-album'), true);
  assert.equal(isSmartCullingSource({ ...localImage, is_cloud_placeholder: true }, null), false);
  assert.equal(isSmartCullingSource({ ...localImage, is_virtual_copy: true }, null), false);
  assert.equal(isSmartCullingSource(localImage, 'immich:album-id'), false);
});

test('a selected cloud placeholder exposes no lock write action', async () => {
  const props = { selectedPaths: [localImage.path], imageList: [{ ...localImage, is_cloud_placeholder: true }] };
  assert.equal(await renderComponent(SmartCullingLockControl, props), '');
  const local = await renderComponent(SmartCullingLockControl, { ...props, imageList: [localImage] });
  assert.ok(local.includes('sc-lock-action'));
});

test('a virtual-copy selection still maps to its local original when a group also contains a cloud placeholder', async () => {
  const original = { ...localImage, group_id: 'asset' };
  const virtual = { ...original, path: `${original.path}?vc=1`, is_virtual_copy: true };
  const cloud = { ...original, path: '/photos/photo.jpg', is_cloud_placeholder: true };
  const html = await renderComponent(SmartCullingLockControl, {
    selectedPaths: [virtual.path],
    imageList: [original, virtual, cloud],
  });
  assert.ok(html.includes('sc-lock-action'));
});

test('the entry disables starting without a local folder and keeps a local folder actionable', async () => {
  const props = { currentFolderPath: null, imageList: [], selectedPaths: [] };
  const unavailable = await renderComponent(SmartCullingEntry, props, 'en');
  assert.ok(unavailable.includes('disabled=""'));
  assert.ok(unavailable.includes('Open a local folder to start Smart Culling'));
  const local = await renderComponent(SmartCullingEntry, { ...props, currentFolderPath: '/photos' }, 'en');
  assert.ok(!local.includes('disabled=""'));
});

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
