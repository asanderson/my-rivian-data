import assert from 'node:assert/strict';
import { test } from 'node:test';

test('release gate requires four matching source revisions and native Windows/macOS signing evidence', async () => {
  const { validateCandidateBuilds } = await import('../lib/release.mjs');
  const revision = 'a'.repeat(40);
  const builds = [
    { platform: 'linux', architecture: 'x64', signed: false, revision, version: '0.1.0' },
    { platform: 'win32', architecture: 'x64', signed: true, revision, version: '0.1.0' },
    { platform: 'darwin', architecture: 'x64', signed: true, revision, version: '0.1.0' },
    { platform: 'darwin', architecture: 'arm64', signed: true, revision, version: '0.1.0' },
  ];
  assert.equal(validateCandidateBuilds(builds, revision), '0.1.0');
  assert.throws(() => validateCandidateBuilds(builds.slice(0, 3), revision), /platform/);
  assert.throws(() => validateCandidateBuilds(builds.map(build => ({ ...build, signed: false })), revision), /signature/);
  assert.throws(() => validateCandidateBuilds(builds.map(build => ({ ...build, revision: 'b'.repeat(40) })), revision), /revision/);
  assert.throws(() => validateCandidateBuilds([...builds, builds[0]], revision), /platform/);
});
