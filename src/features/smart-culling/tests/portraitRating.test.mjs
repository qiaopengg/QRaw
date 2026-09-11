import assert from 'node:assert/strict';
import test from 'node:test';
import { featureModule, renderComponent } from './componentHarness.mjs';

const { PortraitRatingEvidence } = await featureModule('../components/PortraitRatingEvidence.tsx');
const { ReviewInspector } = await featureModule('../components/ReviewInspector.tsx');

function result() {
  return {
    resultId: 'portrait',
    path: '/fixture.jpg',
    source: 'ai',
    mode: 'portrait',
    rating: 3,
    requiresHumanReview: false,
    colorLabel: null,
    groupKind: 'single',
    groupSize: 1,
    reasonCodes: ['portrait_discrete_partial_evidence'],
    keyPersonEvidence: [],
    faces: [
      {
        bbox: [0.1, 0.1, 0.4, 0.4],
        expressionState: 'excellent',
        expressionConfidence: 0.4,
        leftEye: { state: 'open', confidence: 0.8 },
        rightEye: { state: 'closed', confidence: 0.9 },
      },
    ],
    portraitRating: {
      version: 'qraw-portrait-rating-discrete-1.0',
      subjectFaceIndices: [0],
      eyes: 'passed',
      expression: 'excellent',
      opticalAesthetic: { optical: 'unableToDetermine', aestheticComposition: 'unableToDetermine' },
      status: 'scored',
      breakdown: {
        eyeIncrement: 2,
        expressionIncrement: 1,
        opticalAestheticIncrement: 0,
        rawRating: 3,
        finalRating: 3,
      },
    },
  };
}

function render(value, language = 'zh-CN') {
  return renderComponent(PortraitRatingEvidence, { result: value }, language);
}

test('inspector presents backend increments, separate eye states and the five-level expression', async () => {
  const html = await renderComponent(ReviewInspector, { result: result(), onEdit() {} });
  for (const text of [
    '人像评分明细',
    '参与评分的主体',
    '人物左眼',
    '人物右眼',
    '睁眼',
    '闭眼',
    '优秀',
    '+2',
    '+1',
    '3 → 3/5',
    '尚未验证',
  ]) {
    assert.ok(html.includes(text), text);
  }
  assert.ok(!html.includes('sc-expression-pending'));
});

test('every expression label renders in both languages, including unknown and negative totals', async () => {
  const states = [
    ['severeFailure', 'severe_failure', '严重失败', 'Severe failure'],
    ['notRecommended', 'not_recommended', '不推荐', 'Not recommended'],
    ['natural', 'natural', '自然、基本可用', 'Natural, usable'],
    ['excellent', 'excellent', '优秀', 'Excellent'],
    ['outstanding', 'outstanding', '非常出色', 'Outstanding'],
    ['unableToDetermine', 'unknown', '无法确定', 'Unknown'],
  ];
  for (const [aggregate, faceState, zh, en] of states) {
    const value = result();
    value.portraitRating.expression = aggregate;
    value.faces[0].expressionState = faceState;
    for (const [language, text] of [
      ['zh-CN', zh],
      ['en', en],
    ]) {
      const html = await render(value, language);
      assert.ok(html.includes(text));
      assert.ok(!html.includes('undefined'));
    }
  }
  const value = result();
  value.portraitRating.breakdown = {
    eyeIncrement: -2,
    expressionIncrement: -2,
    opticalAestheticIncrement: 0,
    rawRating: -4,
    finalRating: 0,
  };
  value.rating = 0;
  assert.ok((await render(value)).includes('-4 → 0/5'));
});

test('pending and stopped gates expose available signals without displaying stale totals', async () => {
  for (const status of ['pending', 'subjectUnclear']) {
    const value = result();
    value.portraitRating.status = status;
    value.requiresHumanReview = status === 'pending';
    const html = await render(value);
    assert.ok(html.includes('优秀'));
    assert.ok(!html.includes('3 → 3/5'));
    assert.ok(!html.includes('<strong>+2</strong>'));
    assert.ok(html.includes(status === 'pending' ? '本张仍未评分' : '单项结果不再计分'));
  }
  const grouped = result();
  grouped.requiresHumanReview = true;
  assert.ok(!(await render(grouped)).includes('3 → 3/5'));
});

test('manual rating remains visible in inspector without presenting the AI total as current', async () => {
  const value = { ...result(), source: 'manual', rating: 5, protected: true };
  const html = await renderComponent(ReviewInspector, { result: value, onEdit() {} });
  assert.ok(html.includes('人工评分已优先保存'));
  assert.ok(html.includes('优秀'));
  assert.ok(!html.includes('3 → 3/5'));
  assert.equal((html.match(/fill-current/g) ?? []).length, 5);
});

test('missing, unresolved and non-portrait records do not fabricate selected subjects or scores', async () => {
  const absent = result();
  delete absent.portraitRating;
  assert.ok((await render(absent)).includes('本次结果没有逐项评分记录'));
  const unresolved = result();
  unresolved.portraitRating.subjectFaceIndices = [];
  unresolved.portraitRating.status = 'pending';
  assert.ok((await render(unresolved)).includes('评分主体尚未确认'));
  assert.equal(await render({ ...result(), mode: 'landscape' }), '');
});

test('unavailable eye and expression reasons are explained, and invalid confidences never become percentages', async () => {
  const value = result();
  value.faces[0].leftEye = { state: 'unknown', confidence: 0, reason: 'eye_pose_unreliable' };
  value.faces[0].rightEye = null;
  value.faces[0].expressionState = 'unknown';
  value.faces[0].expressionReason = 'expression_model_unvalidated';
  const html = await render(value);
  assert.ok(html.includes('侧脸或姿态使眼睛证据不足'));
  assert.ok(html.includes('没有可用的已验证模型'));
  assert.ok(!html.includes('NaN'));
  value.faces[0].expressionState = 'natural';
  value.faces[0].expressionConfidence = Number.NaN;
  assert.ok(!(await render(value)).includes('NaN'));
});
