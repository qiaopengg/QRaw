import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const require = createRequire(import.meta.url);
export const React = require('react');
const { renderToStaticMarkup } = require('react-dom/server');
const { createInstance } = require('i18next');
const { I18nextProvider } = require('react-i18next');

export async function featureModule(relativePath) {
  const bundled = await build({
    entryPoints: [fileURLToPath(new URL(relativePath, import.meta.url))],
    bundle: true,
    platform: 'node',
    format: 'cjs',
    write: false,
    packages: 'external',
    loader: { '.css': 'empty' },
  });
  const compiled = { exports: {} };
  new Function('require', 'module', 'exports', bundled.outputFiles[0].text)(require, compiled, compiled.exports);
  return compiled.exports;
}

export async function renderComponent(Component, props, language = 'zh-CN') {
  const i18n = createInstance();
  await i18n.init({ lng: language, resources: { [language]: { translation: { fixture: 'ready' } } } });
  return renderToStaticMarkup(React.createElement(I18nextProvider, { i18n }, React.createElement(Component, props)));
}
