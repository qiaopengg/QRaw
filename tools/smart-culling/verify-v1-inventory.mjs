import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const toolDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(toolDir, '../..');
const inventory = JSON.parse(readFileSync(resolve(toolDir, 'v1-inventory.json'), 'utf8'));

function git(args) {
  return execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' }).trim();
}

function walk(directory) {
  return readdirSync(resolve(repoRoot, directory), { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(repoRoot, directory, entry.name);
    return entry.isDirectory() ? walk(relative(repoRoot, path)) : [relative(repoRoot, path)];
  });
}

git(['cat-file', '-e', `${inventory.baselineCommit}^{commit}`]);

const currentFiles = [...walk('src/features/smart-culling'), ...walk('src-tauri/src/features/smart_culling')];
const currentSource = currentFiles
  .filter((path) => /\.(?:rs|ts|tsx)$/.test(path))
  .map((path) => readFileSync(resolve(repoRoot, path), 'utf8'))
  .join('\n');
const failures = [];

const retiredTokens = [
  'smart_culling_analyze',
  'smart_culling_write_metadata',
  'smart-culling-start',
  'smart-culling-progress',
  'smart-culling-complete',
  'delete_files_from_disk',
  "value: 'delete'",
  "'rate_zero'",
];
for (const token of retiredTokens) {
  if (currentSource.includes(token)) failures.push(`retired V1 token is still present: ${token}`);
}

const libSource = readFileSync(resolve(repoRoot, 'src-tauri/src/lib.rs'), 'utf8');
const registered = [...libSource.matchAll(/features::(smart_culling_[a-z_]+)/g)].map((match) => match[1]);
if (JSON.stringify(registered) !== JSON.stringify(['smart_culling_command'])) {
  failures.push(`expected one V2 gateway command, found: ${registered.join(', ') || '(none)'}`);
}
if (!currentSource.includes('smart-culling://event')) {
  failures.push('the single V2 event channel is missing');
}
if (!currentSource.includes('smartCullingV2')) {
  failures.push('featureData.smartCullingV2 persistence key is missing');
}

const appFeatures = readFileSync(resolve(repoRoot, 'src/features/appFeatures.ts'), 'utf8');
if (!appFeatures.includes(inventory.hostRegistration)) {
  failures.push(`host registration ${inventory.hostRegistration} is missing`);
}

const bundledModels = walk('src-tauri/resources/smart_culling_models')
  .filter((path) => path.endsWith('.onnx'))
  .sort();
const requiredV2BundledModels = inventory.requiredV2BundledModels ?? inventory.bundledModels;
if (JSON.stringify(bundledModels) !== JSON.stringify([...requiredV2BundledModels].sort())) {
  failures.push(`bundled V2 model inventory drifted: ${bundledModels.join(', ')}`);
}

if (failures.length > 0) {
  console.error(`V1 retirement verification failed.\n\n${failures.join('\n')}`);
  process.exit(1);
}

console.log(
  `V1 retired: one V2 command, one V2 event, no delete/zero-star/direct V1 command path, ${bundledModels.length} bundled models.`,
);
