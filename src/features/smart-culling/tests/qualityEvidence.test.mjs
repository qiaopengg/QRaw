import assert from 'node:assert/strict';
import test from 'node:test';
import { featureModule, renderComponent } from './componentHarness.mjs';

const { PortraitQualityEvidence } = await featureModule('../components/PortraitQualityEvidence.tsx');

function evidence() {
  return {
    version: 'qraw-portrait-quality-observation-1',
    modelVersion: 'ediffiqa-tiny-jun2024-qraw-observation-1',
    inputSource: 'renderedRgb8',
    inputDimensions: [200, 200],
    subjectFaceIndices: [0],
    optical: {
      highlightClipRatio: 0.1,
      shadowClipRatio: 0,
      meanRgb: [100, 120, 140],
    },
    faces: [
      {
        faceIndex: 0,
        cropDimensions: [120, 120],
        detail: { nativeLaplacian: 32.5, scaledLaplacian: 20.5 },
        optical: null,
        placement: { centerDistance: 0, thirdsDistance: 0.2357, goldenDistance: 0.1669 },
        qualityScore: 0.42,
        qualityUnavailableReason: null,
      },
    ],
  };
}

function render(value, language = 'zh-CN') {
  return renderComponent(PortraitQualityEvidence, { evidence: value }, language);
}

test('measurements clearly remain uncalibrated and raw score is not a percentage', async () => {
  const html = await render(evidence());
  for (const text of ['新质量证据（待校准）', '尚不参与自动打星', '人脸质量原始分', '0.4200', '10.00%']) {
    assert.ok(html.includes(text), text);
  }
  assert.ok(!html.includes('NaN'));
  assert.ok(!html.includes('42.00%'));
});

test('zero quality is a measurement, not unavailable or a star rating', async () => {
  const value = evidence();
  value.faces[0].qualityScore = 0;
  const html = await render(value);
  assert.ok(html.includes('0.0000'));
  assert.ok(html.includes('尚未转为清晰度结论'));
});

test('missing model and missing faces retain explicit unavailable messages', async () => {
  const value = evidence();
  value.faces[0].qualityScore = null;
  value.faces[0].qualityUnavailableReason = 'modelUnavailable';
  const html = await render(value);
  assert.ok(html.includes('质量模型暂不可用'));
  assert.ok(!html.includes('0.0000'));
  const empty = await render({ ...value, faces: [], optical: null });
  assert.ok(empty.includes('未检测到人脸'));
  assert.ok(empty.includes('没有可用的图像观测数据'));
});

test('all unavailable reasons and English copy render without undefined labels', async () => {
  for (const reason of [
    'invalidFaceBox',
    'partialFace',
    'faceTooSmall',
    'unreliableDetection',
    'landmarksUnavailable',
    'strongProfile',
    'modelUnavailable',
    'inferenceFailed',
  ]) {
    const value = evidence();
    value.faces[0].qualityScore = null;
    value.faces[0].qualityUnavailableReason = reason;
    const html = await render(value, 'en');
    assert.ok(html.includes('New quality evidence (uncalibrated)'));
    assert.ok(!html.includes('undefined'));
    assert.ok(!html.includes('0.4200'));
  }
});

function clarity() {
  return {
    version: 'qraw-clarity-input-1',
    inputSource: 'decodedRgb8LosslessGeometry',
    inputDimensions: [200, 200],
    unavailableReason: null,
    faces: [
      {
        faceIndex: 0,
        bbox: [30, 30, 120, 120],
        unavailableReason: null,
        detail: { nativeDimensions: [84, 84], nativeLaplacian: 987.65 },
        robustDetail: { denoisedDetail: { nativeLaplacian: 432.1 } },
      },
    ],
  };
}

test('original and rendered face details stay distinguishable and neither becomes a focus verdict', async () => {
  const html = await render({ ...evidence(), clarity: clarity() });
  for (const text of [
    '原图人脸细节（待校准）',
    '已读取与当前人脸位置对应的原像素',
    '84 × 84',
    '987.65',
    '432.10',
    '32.50',
    '缺少观测不表示照片模糊',
  ])
    assert.ok(html.includes(text), text);
  assert.ok(!html.includes('清晰度通过'));
});

test('unavailable native input never relabels rendered pixels or shows stale native measurements', async () => {
  for (const reason of [
    'notRequested',
    'rawSourceUnvalidated',
    'contentEdited',
    'geometryUnvalidated',
    'invalidAdjustments',
    'invalidCrop',
    'decodeFailed',
    'renderDimensionsMismatch',
  ]) {
    const native = { ...clarity(), unavailableReason: reason };
    for (const language of ['zh-CN', 'en']) {
      const html = await render({ ...evidence(), clarity: native }, language);
      assert.ok(!html.includes('987.65'));
      assert.ok(!html.includes('undefined'));
      assert.ok(html.includes('32.50'));
    }
  }
  const absent = await render(evidence());
  assert.ok(absent.includes('本次没有原图清晰度补充观测'));
  const wrongSource = await render({ ...evidence(), clarity: { ...clarity(), inputSource: 'renderedRgb8' } });
  assert.ok(!wrongSource.includes('987.65'));
});
