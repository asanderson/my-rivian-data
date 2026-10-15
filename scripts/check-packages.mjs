import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { main, root, run } from './lib/process.mjs';
import { sha256, verifyPayload } from './lib/delivery.mjs';
import { startLocalHost } from './lib/local-host.mjs';

main(async () => {
  const args = process.argv.slice(2);
  if (args.length > 1 || args.some(arg => arg !== '--portable-only')) throw new Error('Usage: node scripts/check-packages.mjs [--portable-only]');
  const portableOnly = args.includes('--portable-only');
  const output = path.join(root, 'dist');
  const revision = run('git', ['rev-parse', 'HEAD'], { capture: true }).trim();
  const builds = readdirSync(output).filter(name => {
    if (!name.endsWith('.BUILD.json')) return false;
    const build = JSON.parse(readFileSync(path.join(output, name), 'utf8'));
    return build.revision === revision && build.platform === process.platform && build.architecture === process.arch;
  });
  if (builds.length !== 1) throw new Error('Package checks require exactly one current-revision build for this platform in dist.');
  const name = builds[0].replace(/\.BUILD\.json$/, '');
  const build = JSON.parse(readFileSync(path.join(output, builds[0]), 'utf8'));
  const checksums = readFileSync(path.join(output, `${name}.SHA256SUMS`), 'utf8').trim().split('\n');
  for (const line of checksums) {
    const match = /^([a-f0-9]{64})  ([A-Za-z0-9._-]+)$/.exec(line);
    if (!match || sha256(path.join(output, match[2])) !== match[1]) throw new Error('Artifact checksum mismatch.');
  }
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'my-rivian-data-package-check-'));
  try {
    run('tar', ['-xzf', path.join(output, `${name}.tar.gz`), '-C', temporary]);
    const portable = path.join(temporary, name);
    verifyPayload(portable);
    assert.equal(run(path.join(portable, build.executable), ['--version'], { capture: true }).trim(), `My Rivian Data ${build.version}`);
    // Exercise the extracted executable, including embedded assets and native
    // protected execution. No owner account or OS credential store is used.
    const host = await startLocalHost({ binary: path.join(portable, build.executable), demo: true });
    try {
      const index = await fetch(`${host.origin}/`, { signal: AbortSignal.timeout(5000) });
      assert.equal(index.status, 200);
      const html = await index.text();
      const script = html.match(/src="(\/assets\/[A-Za-z0-9._-]+\.js)"/u)?.[1];
      assert.ok(script, 'Embedded index must reference its compiled JavaScript');
      for (const asset of [script, '/wasm/rivian_wasm.js', '/wasm/rivian_wasm_bg.wasm']) {
        const response = await fetch(`${host.origin}${asset}`, { signal: AbortSignal.timeout(5000) });
        assert.equal(response.status, 200, asset);
        assert.ok((await response.arrayBuffer()).byteLength > 0, asset);
      }
      assert.equal((await (await host.demoSession()).executeVehicleState()).status, 200);
    } finally { await host.stop(); }
    if (portableOnly) {
      console.log('Artifact hashes, extracted payload, embedded UI/WASM and protected demo execution passed; native installer checks were explicitly skipped.');
      return;
    }
    if (process.platform === 'win32') {
      run('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', path.join(root, 'packaging/windows/check-install.ps1'), '-Installer', path.join(output, `${name}-setup.exe`), '-Version', build.version, '-Revision', build.revision.slice(0, 12)]);
    } else if (process.platform === 'darwin') {
      const mount = path.join(temporary, 'mount');
      run('hdiutil', ['attach', '-readonly', '-nobrowse', '-mountpoint', mount, path.join(output, `${name}.dmg`)]);
      try {
        const app = path.join(mount, 'My Rivian Data.app');
        run('plutil', ['-lint', path.join(app, 'Contents/Info.plist')]);
        assert.equal(run(path.join(app, 'Contents/MacOS/my-rivian-data'), ['--version'], { capture: true }).trim(), `My Rivian Data ${build.version}`);
        verifyPayload(path.join(app, 'Contents/Resources'));
        if (build.signed) run('codesign', ['--verify', '--deep', '--strict', app]);
      } finally { run('hdiutil', ['detach', mount]); }
    } else if (process.platform === 'linux') {
      const deb = path.join(output, `${name}.deb`), rpm = path.join(output, `${name}.rpm`);
      if (!existsSync(deb) || !existsSync(rpm)) throw new Error('Missing DEB/RPM.');
      run('desktop-file-validate', [path.join(root, 'packaging/linux/my-rivian-data.desktop')]);
      run('dpkg-deb', ['--info', deb]);
      // Isolated extraction and RPM database avoid installing on the owner's OS.
      const debRoot = path.join(temporary, 'deb');
      run('dpkg-deb', ['--extract', deb, debRoot]);
      verifyPayload(path.join(debRoot, 'usr/share/my-rivian-data'));
      assert.equal(run(path.join(debRoot, 'usr/bin/my-rivian-data'), ['--version'], { capture: true }).trim(), `My Rivian Data ${build.version}`);
      const rpmRoot = path.join(temporary, 'rpm');
      run('rpm', ['--root', rpmRoot, '--initdb']);
      run('rpm', ['--root', rpmRoot, '--install', '--nodeps', '--noscripts', rpm]);
      run('rpm', ['--root', rpmRoot, '--verify', 'my-rivian-data']);
      verifyPayload(path.join(rpmRoot, 'usr/share/my-rivian-data'));
      run('rpm', ['--root', rpmRoot, '--erase', '--nodeps', '--noscripts', 'my-rivian-data']);
      assert.equal(existsSync(path.join(rpmRoot, 'usr/bin/my-rivian-data')), false);
    }
    console.log('Artifact hashes, portable execution and native package structure/lifecycle checks passed on this host.');
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});
