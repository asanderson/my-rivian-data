import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { main, root, run } from './lib/process.mjs';
import { fileManifest, sha256, verifyPayload } from './lib/delivery.mjs';
import { createSboms } from './lib/sbom.mjs';
import { nativePackages } from './lib/native-packages.mjs';
import { prepareSigning, signMac, signWindows } from './lib/signing.mjs';

main(() => {
  const version = readFileSync(path.join(root, 'Cargo.toml'), 'utf8').match(/\[workspace\.package\][\s\S]*?version = "([^"]+)"/u)?.[1];
  if (!version) throw new Error('Cannot find the workspace package version.');
  const executable = process.platform === 'win32' ? 'my-rivian-data.exe' : 'my-rivian-data';
  const binary = path.join(root, 'target/release', executable);
  const args = process.argv.slice(2);
  if (args.some(arg => !['--signed', '--portable-only'].includes(arg))) throw new Error('Usage: node scripts/package.mjs [--signed] [--portable-only]');
  const signed = args.includes('--signed');
  if (signed && args.includes('--portable-only')) throw new Error('Signed candidates require native installer/app verification; --portable-only is development-only.');
  if (signed && !['win32', 'darwin'].includes(process.platform)) throw new Error('Linux authenticity uses the separately signed release checksum manifest, not native code signing.');
  // The included corresponding source must match the revision being distributed.
  // Avoid silently packaging HEAD while the executable came from uncommitted code.
  if (run('git', ['status', '--porcelain', '--untracked-files=normal'], { capture: true }).trim()) {
    throw new Error('Commit the source changes before packaging so the source archive matches the build.');
  }
  const revision = run('git', ['rev-parse', 'HEAD'], { capture: true }).trim();
  // Rebuild from the just-checked revision: a clean tree alone does not prove
  // an existing executable came from it. The build uses Cargo/npm incremental caches.
  run(process.execPath, ['scripts/build.mjs']);
  if (!existsSync(binary)) throw new Error('Release rebuild produced no executable.');
  if (run('git', ['status', '--porcelain', '--untracked-files=normal'], { capture: true }).trim()
      || run('git', ['rev-parse', 'HEAD'], { capture: true }).trim() !== revision) {
    throw new Error('Source changed while building. Commit and rerun packaging.');
  }
  const name = `my-rivian-data-${version}-${process.platform}-${process.arch}`;
  const output = path.join(root, 'dist');
  const staging = path.join(output, name);
  if (existsSync(output)) {
    for (const filename of readdirSync(output)) {
      if (filename.startsWith(`${name}.`) || filename.startsWith(`${name}-setup.exe`)) rmSync(path.join(output, filename), { force: true });
    }
  }
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging, { recursive: true });
  copyFileSync(binary, path.join(staging, executable));
  if (process.platform !== 'win32') chmodSync(path.join(staging, executable), 0o755);
  copyFileSync(path.join(root, 'LICENSE'), path.join(staging, 'LICENSE'));
  cpSync(path.join(root, 'docs'), path.join(staging, 'docs'), { recursive: true });
  copyFileSync(path.join(root, 'README.md'), path.join(staging, 'README.md'));
  if (process.platform === 'win32') copyFileSync(path.join(root, 'packaging/windows/verify.ps1'), path.join(staging, 'verify.ps1'));
  writeFileSync(path.join(staging, 'START-HERE.txt'), [
    'My Rivian Data — Your vehicles. Your data.',
    '',
    `${signed ? 'Release candidate (signing requested; inspect completed release manifest)' : 'Unsigned development build'}: ${process.platform}/${process.arch}, version ${version}`,
    `Source revision: ${revision}`,
    '',
    'Extract this entire package before running it.',
    process.platform === 'win32'
      ? 'Open PowerShell in this folder and run: .\\my-rivian-data.exe'
      : 'Open a terminal in this folder and run: ./my-rivian-data',
    'The app opens a local page in your default browser. Keep the terminal open.',
    'Sign in there with your Rivian account; complete verification if requested.',
    'Owner data access and explicitly confirmed vehicle nickname changes.',
    'Passwords are never saved. Remember sign-in is optional and uses the native OS vault.',
    'For fictional data without signing in, add --demo to the command.',
    'Choose End session when finished, then press Ctrl+C in the terminal.',
    '',
    'See docs/OWNER-GUIDE.md for help and docs/STATUS.md for validation limits.',
    signed ? 'Use only candidates with a completed signed release manifest; verify its publisher before installation.' : 'This build is unsigned. Hashes detect file changes; they do not prove publisher identity.',
    'GPL-3.0-only project; dependency notices are under docs/dependency-licenses/.',
    'Corresponding project source and build scripts are included in source.tar.gz.',
    '',
  ].join('\n'));
  run('git', ['archive', '--format=tar.gz', '--prefix=my-rivian-data-source/', `--output=${path.join(staging, 'source.tar.gz')}`, revision]);
  let glibcFloor;
  if (process.platform === 'linux') {
    const versions = [...run('readelf', ['--version-info', binary], { capture: true }).matchAll(/GLIBC_(\d+\.\d+)/g)].map(match => match[1]);
    versions.sort((left, right) => Number(left.split('.')[0]) - Number(right.split('.')[0]) || Number(left.split('.')[1]) - Number(right.split('.')[1]));
    glibcFloor = versions.at(-1);
    if (!glibcFloor) throw new Error('Cannot establish the Linux GNU libc symbol floor.');
    if (!args.includes('--portable-only') && (Number(glibcFloor.split('.')[0]) > 2 || Number(glibcFloor.split('.')[1]) > 35)) throw new Error(`Linux native packaging requires the Ubuntu 22.04/glibc 2.35 baseline; this binary requires ${glibcFloor}.`);
  }
  const cleanup = signed ? prepareSigning() : () => {};
  try {
    if (signed) {
      if (process.platform === 'win32') signWindows(path.join(staging, executable));
      else signMac(path.join(staging, executable), 'binary');
    }
    const sboms = createSboms({ version, revision,
      cargoLock: readFileSync(path.join(root, 'Cargo.lock'), 'utf8'),
      npmLock: JSON.parse(readFileSync(path.join(root, 'ui/package-lock.json'), 'utf8')),
      notices: readFileSync(path.join(root, 'docs/THIRD-PARTY-NOTICES.md'), 'utf8'),
    });
    const artifacts = [];
    for (const [format, content] of Object.entries(sboms)) {
      const filename = `SBOM.${format}.json`;
      writeFileSync(path.join(staging, filename), JSON.stringify(content, null, 2) + '\n');
      const external = path.join(output, `${name}.${filename}`);
      copyFileSync(path.join(staging, filename), external); artifacts.push(external);
    }
    const provenance = { version, revision, platform: process.platform, architecture: process.arch, executable,
      sha256: sha256(path.join(staging, executable)), signed,
      signature: signed ? (process.platform === 'darwin' ? 'Contained executable Developer ID signature verified; app/DMG notarization must complete before the checksum manifest exists' : 'Contained executable Authenticode SHA256 verified; installer/uninstaller verification must complete before the checksum manifest exists') : 'unsigned-development',
      source: { file: 'source.tar.gz', sha256: sha256(path.join(staging, 'source.tar.gz')) },
      locks: { cargo: sha256(path.join(root, 'Cargo.lock')), npm: sha256(path.join(root, 'ui/package-lock.json')) },
      tools: { rust: run('rustc', ['--version'], { capture: true }).trim(), node: process.version },
      ...(process.platform === 'linux' ? { compatibility: `x64 GNU/Linux, observed glibc symbol floor ${glibcFloor}; native packages require glibc >= 2.35. Desktop browser and xdg-utils required.` } : {}),
      ci: process.env.GITHUB_ACTIONS === 'true' ? { repository: process.env.GITHUB_REPOSITORY, workflow: process.env.GITHUB_WORKFLOW, runId: process.env.GITHUB_RUN_ID } : null,
      reproducibility: 'Clean HEAD and locked dependencies; rebuild required. Byte-for-byte reproducibility is not claimed.',
    };
    writeFileSync(path.join(staging, 'BUILD.json'), JSON.stringify(provenance, null, 2) + '\n');
    writeFileSync(path.join(staging, 'FILES.json'), JSON.stringify(fileManifest(staging), null, 2) + '\n');
    verifyPayload(staging);
    const externalBuild = path.join(output, `${name}.BUILD.json`);
    copyFileSync(path.join(staging, 'BUILD.json'), externalBuild); artifacts.push(externalBuild);
    const archive = path.join(output, `${name}.tar.gz`);
    // Windows 10/11 and supported GitHub images provide bsdtar; macOS/Linux tar.
    // Separate arguments avoid shell expansion.
    run('tar', ['-czf', archive, '-C', output, name]); artifacts.push(archive);
    if (!args.includes('--portable-only')) artifacts.push(...nativePackages({ staging, output, name, version, revision, signed }));
    // Failed native signing/notarization never reaches a complete manifest.
    const checksums = artifacts.map(file => `${sha256(file)}  ${path.basename(file)}\n`).join('');
    for (const artifact of artifacts) writeFileSync(`${artifact}.sha256`, `${sha256(artifact)}  ${path.basename(artifact)}\n`);
    writeFileSync(path.join(output, `${name}.SHA256SUMS`), checksums);
    console.log(`\n${signed ? 'Native signed release candidate' : 'Unsigned development packages'} created in ${output}`);
  } finally { cleanup(); }
});
