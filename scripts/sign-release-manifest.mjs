import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { main, root, run } from './lib/process.mjs';
import { sha256 } from './lib/delivery.mjs';
import { validateCandidateBuilds } from './lib/release.mjs';

main(() => {
  if (!process.env.RIVIAN_RELEASE_GPG_KEY || !process.env.RIVIAN_RELEASE_GPG_PASSPHRASE) throw new Error('Signed release manifest blocked: GPG CI secrets are required.');
  if (process.argv.length !== 3) throw new Error('Use sign-release-manifest.mjs LOCAL_CANDIDATE_DIRECTORY.');
  const directory = path.resolve(root, process.argv[2]);
  const names = readdirSync(directory).sort();
  if (names.some(name => !/^[A-Za-z0-9._-]+$/.test(name))) throw new Error('Unexpected release file names.');
  const revision = run('git', ['rev-parse', 'HEAD'], { capture: true }).trim();
  const builds = names.filter(name => name.endsWith('.BUILD.json')).map(name => JSON.parse(readFileSync(path.join(directory, name), 'utf8')));
  const version = validateCandidateBuilds(builds, revision);
  for (const name of names.filter(file => file.endsWith('.SHA256SUMS'))) {
    for (const line of readFileSync(path.join(directory, name), 'utf8').trim().split('\n')) {
      const match = /^([a-f0-9]{64})  ([A-Za-z0-9._-]+)$/.exec(line);
      if (!match || sha256(path.join(directory, match[2])) !== match[1]) throw new Error('Candidate artifact integrity check failed.');
    }
  }
  const manifest = path.join(directory, 'RELEASE-SHA256SUMS');
  writeFileSync(manifest, names.map(name => `${sha256(path.join(directory, name))}  ${name}\n`).join(''));
  writeFileSync(path.join(directory, 'RELEASE.json'), JSON.stringify({ version, revision, platforms: builds.map(build => `${build.platform}/${build.architecture}`), status: 'signed-candidate-not-published', checksumManifest: 'RELEASE-SHA256SUMS', unsignedNativeLinux: 'Authenticity is provided by the detached checksum-manifest signature.' }, null, 2) + '\n');
  // Include release metadata in the signed checksum manifest too.
  writeFileSync(manifest, readFileSync(manifest, 'utf8') + `${sha256(path.join(directory, 'RELEASE.json'))}  RELEASE.json\n`);
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'my-rivian-data-release-key-'));
  const gpg = (args, input) => {
    const result = spawnSync('gpg', ['--homedir', temporary, '--batch', ...args], { shell: false, encoding: 'utf8', input, stdio: ['pipe', 'pipe', 'pipe'] });
    if (result.error || result.status !== 0) throw new Error('Release manifest signing/verification failed; no publishable candidate.');
    return result.stdout;
  };
  try {
    const key = path.join(temporary, 'private-key.asc');
    writeFileSync(key, process.env.RIVIAN_RELEASE_GPG_KEY, { mode: 0o600 });
    gpg(['--import', key]);
    const keys = gpg(['--with-colons', '--list-secret-keys']);
    if (keys.split('\n').filter(line => line.startsWith('sec:')).length !== 1) throw new Error('Exactly one release signing key is required.');
    const fingerprint = keys.split('\n').find(line => line.startsWith('fpr:'))?.split(':')[9];
    if (!/^[A-F0-9]{40,64}$/.test(fingerprint ?? '')) throw new Error('No release signing key fingerprint.');
    gpg(['--yes', '--pinentry-mode', 'loopback', '--passphrase-fd', '0', '--local-user', fingerprint, '--armor', '--detach-sign', '--output', `${manifest}.asc`, manifest], `${process.env.RIVIAN_RELEASE_GPG_PASSPHRASE}\n`);
    gpg(['--verify', `${manifest}.asc`, manifest]);
    writeFileSync(path.join(directory, 'RELEASE-PUBLIC-KEY.asc'), gpg(['--armor', '--export', fingerprint]));
    console.log(`Verified signed release candidate; GPG fingerprint ${fingerprint}. No release was created or published.`);
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});
