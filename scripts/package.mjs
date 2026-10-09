import { createHash } from 'node:crypto';
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { main, root, run } from './lib/process.mjs';

main(() => {
  const version = readFileSync(path.join(root, 'Cargo.toml'), 'utf8').match(/\[workspace\.package\][\s\S]*?version = "([^"]+)"/u)?.[1];
  if (!version) throw new Error('Cannot find the workspace package version.');
  const executable = process.platform === 'win32' ? 'my-rivian-data.exe' : 'my-rivian-data';
  const binary = path.join(root, 'target/release', executable);
  if (!existsSync(binary)) throw new Error('Build the release executable before packaging.');
  // The included corresponding source must match the revision being distributed.
  // Avoid silently packaging HEAD while the executable came from uncommitted code.
  if (run('git', ['status', '--porcelain', '--untracked-files=normal'], { capture: true }).trim()) {
    throw new Error('Commit the source changes before packaging so the source archive matches the build.');
  }
  const revision = run('git', ['rev-parse', 'HEAD'], { capture: true }).trim();
  // Rebuild from the just-checked revision: a clean tree alone does not prove
  // an existing executable came from it. The build uses Cargo/npm incremental caches.
  run(process.execPath, ['scripts/build.mjs']);
  if (run('git', ['status', '--porcelain', '--untracked-files=normal'], { capture: true }).trim()
      || run('git', ['rev-parse', 'HEAD'], { capture: true }).trim() !== revision) {
    throw new Error('Source changed while building. Commit and rerun packaging.');
  }
  const name = `my-rivian-data-${version}-${process.platform}-${process.arch}`;
  const output = path.join(root, 'dist');
  const staging = path.join(output, name);
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging, { recursive: true });
  copyFileSync(binary, path.join(staging, executable));
  if (process.platform !== 'win32') chmodSync(path.join(staging, executable), 0o755);
  copyFileSync(path.join(root, 'LICENSE'), path.join(staging, 'LICENSE'));
  cpSync(path.join(root, 'docs'), path.join(staging, 'docs'), { recursive: true });
  copyFileSync(path.join(root, 'README.md'), path.join(staging, 'README.md'));
  writeFileSync(path.join(staging, 'START-HERE.txt'), [
    'My Rivian Data — Your vehicles. Your data.',
    '',
    `Unsigned development build: ${process.platform}/${process.arch}, version ${version}`,
    `Source revision: ${revision}`,
    '',
    'Extract this entire package before running it.',
    process.platform === 'win32'
      ? 'Open PowerShell in this folder and run: .\\my-rivian-data.exe'
      : 'Open a terminal in this folder and run: ./my-rivian-data',
    'The app opens a local page in your default browser. Keep the terminal open.',
    'Sign in there with your Rivian account; complete verification if requested.',
    'Read-only access. Account passwords and Rivian tokens are not saved to disk.',
    'For fictional data without signing in, add --demo to the command.',
    'Choose End session when finished, then press Ctrl+C in the terminal.',
    '',
    'See docs/OWNER-GUIDE.md for help and docs/STATUS.md for validation limits.',
    'These builds are unsigned. Hashes detect file changes; they do not prove publisher identity.',
    'GPL-3.0-only project; dependency notices are under docs/dependency-licenses/.',
    'Corresponding project source and build scripts are included in source.tar.gz.',
    '',
  ].join('\n'));
  run('git', ['archive', '--format=tar.gz', '--prefix=my-rivian-data-source/', `--output=${path.join(staging, 'source.tar.gz')}`, revision]);
  const binaryHash = createHash('sha256').update(readFileSync(binary)).digest('hex');
  writeFileSync(path.join(staging, 'BUILD.json'), `${JSON.stringify({ version, revision, platform: process.platform, architecture: process.arch, executable, sha256: binaryHash, signed: false }, null, 2)}\n`);
  const archive = path.join(output, `${name}.tar.gz`);
  // Windows 10/11 and the supported GitHub-hosted images provide bsdtar; macOS
  // and Linux provide tar. Passing arguments separately avoids shell expansion.
  run('tar', ['-czf', archive, '-C', output, name]);
  const archiveHash = createHash('sha256').update(readFileSync(archive)).digest('hex');
  writeFileSync(`${archive}.sha256`, `${archiveHash}  ${path.basename(archive)}\n`);
  console.log(`\nUnsigned package: ${archive}\nSHA-256: ${archiveHash}`);
});
