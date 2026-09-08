import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

// Exercise the actual component without adding a DOM/test framework dependency.
const require = createRequire(import.meta.url);
// Use the same CJS instances as the bundle, including its i18next context.
const React = require('react');
const { renderToStaticMarkup } = require('react-dom/server');
const { createInstance } = require('i18next');
const { I18nextProvider } = require('react-i18next');
const bundled = await build({
  entryPoints: [fileURLToPath(new URL('../components/PortraitQualityEvidence.tsx', import.meta.url))],
  bundle: true,
  platform: 'node',
  format: 'cjs',
  write: false,
  packages: 'external',
  loader: { '.css': 'empty' },
});
const compiled = { exports: {} };
new Function('require', 'module', 'exports', bundled.outputFiles[0].text)(require, compiled, compiled.exports);
const { PortraitQualityEvidence } = compiled.exports;

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

async function render(value, language = 'zh-CN') {
  const i18n = createInstance();
  await i18n.init({ lng: language, resources: { [language]: { translation: { fixture: 'ready' } } } });
  return renderToStaticMarkup(
    React.createElement(I18nextProvider, { i18n }, React.createElement(PortraitQualityEvidence, { evidence: value })),
  );
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
