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

const changed = new Set([
  ...lines(git(['diff', '--name-only', baseline.taskBaseCommit])),
  ...lines(git(['ls-files', '--others', '--exclude-standard'])),
]);

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
  const ranges = baseline.mixedOwnershipRules?.[path]?.allowedBaseLineRanges ?? [];
  const diff = git(['diff', '--unified=0', baseline.taskBaseCommit, '--', path]);
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
