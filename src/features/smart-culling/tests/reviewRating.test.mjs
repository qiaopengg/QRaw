import assert from 'node:assert/strict';
import test from 'node:test';
import { featureModule, renderComponent, React } from './componentHarness.mjs';

const { Stars } = await featureModule('../components/ReviewControls.tsx');
const { reviewResultBucket, reviewResultIsWritable, reviewChangeFor } = await featureModule('../reviewPolicy.ts');
const { useSmartCullingReasonText } = await featureModule('../i18n.ts');
const result = (rating, requiresHumanReview = false, source = 'ai') => ({
  resultId: 'test-result',
  rating,
  requiresHumanReview,
  source,
  colorLabel: null,
  mode: 'portrait',
});

test('valid zero, positive ratings and unresolved placeholders occupy different result buckets', () => {
  assert.equal(reviewResultBucket(result(0)), 'aiRejected');
  for (let rating = 1; rating <= 5; rating++) assert.equal(reviewResultBucket(result(rating)), 'aiRated');
  for (const rating of [0, 4]) {
    assert.equal(reviewResultBucket(result(rating, true)), 'pending');
    assert.equal(reviewResultIsWritable(result(rating, true)), false);
  }
  assert.equal(reviewResultBucket(result(0, false, 'manual')), 'aiRejected');
  assert.equal(reviewResultIsWritable(result(0, false, 'manual')), true);
});

test('an explicit manual decision leaves the pending queue while preserving its rating bucket', () => {
  for (let rating = 0; rating <= 5; rating++) {
    const manual = { ...result(rating, false, 'manual'), locked: true };
    assert.equal(reviewResultBucket(manual), rating === 0 ? 'aiRejected' : 'aiRated');
    assert.equal(reviewResultIsWritable(manual), true);
    assert.equal(manual.locked, true);
  }
});

test('unresolved zero and stale positive ratings display Unrated, while valid zero remains explicit', async () => {
  for (const rating of [0, 4]) {
    const html = await renderComponent(Stars, { result: result(rating, true), compact: true });
    assert.ok(html.includes('未评分'));
    assert.ok(!html.includes('fill-current'));
    assert.ok(!html.includes('0 星'));
  }
  const zero = await renderComponent(Stars, { result: result(0) });
  assert.ok(zero.includes('aria-label="0 星级"'));
  assert.ok(!zero.includes('未评分'));
  const manual = await renderComponent(Stars, { result: result(4, false, 'manual') });
  assert.equal((manual.match(/fill-current/g) ?? []).length, 4);
});

test('an unrated photo has an explicit manual zero choice without retaining stale highlighted stars', async () => {
  const html = await renderComponent(Stars, { result: result(4, true), onChange() {} }, 'en');
  assert.ok(html.includes('Unrated'));
  assert.ok(html.includes('aria-label="0 star rating"'));
  assert.ok(!html.includes('fill-current'));
  assert.ok(!html.includes('disabled'));
});

test('a color-only edit cannot submit an unresolved placeholder rating', () => {
  for (const rating of [0, 4]) {
    const before = result(rating, true);
    assert.equal(reviewChangeFor(before, { colorLabel: 'green' }), null);
    assert.deepEqual(before, result(rating, true));
    assert.equal(reviewChangeFor(before, { rating: 0 }).rating, 0);
    assert.equal(reviewChangeFor(before, { rating: 3 }).rating, 3);
  }
  assert.deepEqual(reviewChangeFor(result(4), { colorLabel: 'red' }), {
    resultId: 'test-result',
    rating: 4,
    colorLabel: 'red',
  });
  assert.equal(reviewChangeFor(result(0, false, 'manual'), { colorLabel: 'green' }).rating, 0);
});

test('portrait unknown reasons never describe an effective zero-star result', async () => {
  function Reasons({ codes }) {
    return React.createElement('p', null, useSmartCullingReasonText()(codes));
  }
  for (const code of ['portrait_key_person_unresolved', 'portrait_person_clarity_unresolved', 'key_person_1_missing']) {
    const zh = await renderComponent(Reasons, { codes: [code] });
    const en = await renderComponent(Reasons, { codes: [code] }, 'en');
    assert.ok(zh.includes('未生成有效评分'));
    assert.ok(!zh.includes('0 星'));
    assert.ok(en.includes('unrated'));
    assert.ok(!en.includes('0 stars'));
  }
});
