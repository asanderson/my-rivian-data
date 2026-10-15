export function validateCandidateBuilds(builds, revision) {
  const expected = ['darwin/arm64', 'darwin/x64', 'linux/x64', 'win32/x64'];
  const actual = builds.map(build => `${build.platform}/${build.architecture}`).sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('Release requires the complete four-platform matrix.');
  if (builds.some(build => build.revision !== revision)) throw new Error('Release source revision mismatch.');
  if (builds.some(build => build.platform !== 'linux' && build.signed !== true)) throw new Error('Native signature evidence is required for Windows/macOS release candidates.');
  const versions = new Set(builds.map(build => build.version));
  if (versions.size !== 1 || !/^\d+\.\d+\.\d+$/.test([...versions][0])) throw new Error('Release versions must match.');
  return [...versions][0];
}
