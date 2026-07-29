import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const toolDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(toolDir, '../..');
const baselinePath = resolve(toolDir, 'ownership-baseline.json');
const baseline = JSON.parse(readFileSync(baselinePath, 'utf8'));

function git(args) {
  return execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' }).trim();
}

function gitOrEmpty(args) {
  try {
    return git(args);
  } catch {
    return '';
  }
}

function lines(value) {
  return value
    ? value
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean)
    : [];
}

function isAllowed(path) {
  return baseline.allowedPathPrefixes.some((prefix) => path.startsWith(prefix));
}

git(['cat-file', '-e', `${baseline.taskBaseCommit}^{commit}`]);
git(['cat-file', '-e', `${baseline.upstreamCommit}^{commit}`]);
git(['cat-file', '-e', `${baseline.featureTipCommit}^{commit}`]);

const changed = new Set(lines(git(['diff', '--name-only', baseline.taskBaseCommit, baseline.featureTipCommit])));
const postFeatureChanges = new Set();
const postFeatureCommits = lines(
  git(['rev-list', '--reverse', '--first-parent', '--no-merges', `${baseline.featureTipCommit}..HEAD`]),
);

for (const commit of postFeatureCommits) {
  const parent = git(['rev-parse', `${commit}^`]);
  for (const path of lines(git(['diff', '--name-only', parent, commit]))) {
    changed.add(path);
    postFeatureChanges.add(path);
  }
}

const mergeInProgress = Boolean(gitOrEmpty(['rev-parse', '--verify', '-q', 'MERGE_HEAD']));
if (!mergeInProgress) {
  for (const path of lines(git(['diff', '--name-only']))) {
    changed.add(path);
    postFeatureChanges.add(path);
  }
  for (const path of lines(git(['ls-files', '--others', '--exclude-standard']))) {
    changed.add(path);
    postFeatureChanges.add(path);
  }
}

const frozenChanges = [...changed].filter((path) => baseline.frozenPaths.includes(path));
const mixedChanges = [...changed].filter((path) => baseline.mixedOwnershipPaths.includes(path));
const outsideAllowlist = [...changed].filter(
  (path) => !isAllowed(path) && !baseline.mixedOwnershipPaths.includes(path),
);

const failures = [];
if (frozenChanges.length > 0) {
  failures.push(`frozen host files changed:\n  ${frozenChanges.join('\n  ')}`);
}
if (outsideAllowlist.length > 0) {
  failures.push(`paths outside the smart-culling allowlist changed:\n  ${outsideAllowlist.join('\n  ')}`);
}
if (mixedChanges.length > 0 && baseline.approvedMixedOwnershipPatchIds.length === 0) {
  failures.push(`mixed-ownership host files changed without an approved patch id:\n  ${mixedChanges.join('\n  ')}`);
}

for (const path of mixedChanges) {
  if (postFeatureChanges.has(path)) {
    failures.push(`mixed-ownership host file changed after the approved feature tip:\n  ${path}`);
    continue;
  }
  const ranges = baseline.mixedOwnershipRules?.[path]?.allowedBaseLineRanges ?? [];
  const diff = git(['diff', '--unified=0', baseline.taskBaseCommit, baseline.featureTipCommit, '--', path]);
  const hunks = [...diff.matchAll(/^@@ -(\d+)(?:,(\d+))? \+\d+(?:,\d+)? @@/gm)].map((match) => ({
    start: Number(match[1]),
    count: match[2] === undefined ? 1 : Number(match[2]),
  }));
  const outsideOwnedLines = hunks.filter(({ start, count }) => {
    const end = count === 0 ? start : start + count - 1;
    return !ranges.some(([rangeStart, rangeEnd]) => start >= rangeStart && end <= rangeEnd);
  });
  if (hunks.length === 0 || outsideOwnedLines.length > 0) {
    failures.push(
      `mixed-ownership hunks escaped approved base-line ranges in ${path}:\n  ${
        outsideOwnedLines.length > 0
          ? outsideOwnedLines.map(({ start, count }) => `-${start},${count}`).join('\n  ')
          : 'unable to parse diff hunks'
      }`,
    );
  }
}

if (failures.length > 0) {
  console.error(`Smart-culling boundary verification failed.\n\n${failures.join('\n\n')}`);
  process.exit(1);
}

console.log(`Smart-culling boundary verified: ${changed.size} changed path(s), all owned by the feature allowlist.`);
