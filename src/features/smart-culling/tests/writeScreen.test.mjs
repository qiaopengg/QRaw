import assert from 'node:assert/strict';
import test from 'node:test';
import { featureModule, renderComponent } from './componentHarness.mjs';

const { WriteScreen } = await featureModule('../components/WriteScreen.tsx');
const snapshot = {
  state: 'completed',
  writeSummary: { succeeded: 1, failed: 1, protected: 0, skipped: 0, succeededPaths: [] },
  failures: [{ path: '/photos/failed.rrsidecar', stage: 'write', code: 'io', detail: 'IO failed', retryable: true }],
};

test('write retry keeps all navigation and repeat-write actions disabled until completion', async () => {
  const props = { snapshot, onExit() {} };
  const completed = await renderComponent(WriteScreen, props, 'en');
  assert.equal((completed.match(/<button\b/g) ?? []).length, 3);
  assert.ok(!completed.includes('disabled=""'));

  const confirming = await renderComponent(
    WriteScreen,
    { ...props, snapshot: { ...snapshot, state: 'confirming' } },
    'en',
  );
  const buttons = confirming.match(/<button\b[^>]*>/g) ?? [];
  assert.equal(buttons.length, 3);
  for (const button of buttons) assert.ok(button.includes('disabled=""'), button);
});
