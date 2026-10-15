import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, existsSync, copyFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// These cases fail if lock hashes are guessed, a changed payload is accepted,
// or account acceptance records permit extra identity/credential fields.
test('SBOM preserves each locked Cargo/npm archive digest and provenance', async () => {
  const { createSboms } = await import('../lib/sbom.mjs');
  const result = createSboms({ version: '1.0.0', revision: 'a'.repeat(40),
    cargoLock: '[[package]]\nname = "example"\nversion = "1.2.3"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "' + 'b'.repeat(64) + '"\n',
    npmLock: { packages: { 'node_modules/@example/module': { version: '2.0.0', resolved: 'https://registry.npmjs.org/example.tgz', integrity: 'sha512-' + Buffer.alloc(64, 9).toString('base64'), license: 'MIT', dev: true } } },
    notices: '| example | 1.2.3 | MIT OR Apache-2.0 | files |' });
  assert.equal(result.cyclonedx.components.length, 2);
  assert.equal(result.cyclonedx.components[0].hashes[0].content, 'b'.repeat(64));
  assert.equal(result.cyclonedx.components[1].hashes[0].content, Buffer.alloc(64, 9).toString('hex'));
  assert.equal(result.spdx.packages[1].downloadLocation, 'https://crates.io/api/v1/crates/example/1.2.3/download');
  assert.equal(result.spdx.packages[2].checksums[0].algorithm, 'SHA512');
  assert.equal(result.cyclonedx.components[1].scope, 'excluded');
});

test('SBOM fails closed on missing registry digest', async () => {
  const { createSboms } = await import('../lib/sbom.mjs');
  assert.throws(() => createSboms({ version: '1', revision: 'a'.repeat(40), cargoLock: '[[package]]\nname = "x"\nversion = "1"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n', npmLock: { packages: {} }, notices: '' }), /checksum/);
});

test('payload verification rejects modifications, extra files and symlinks', async () => {
  const { fileManifest, verifyPayload } = await import('../lib/delivery.mjs');
  const directory = mkdtempSync(path.join(os.tmpdir(), 'rivian-delivery-'));
  try {
    mkdirSync(path.join(directory, 'docs')); writeFileSync(path.join(directory, 'docs', 'notice.txt'), 'notice');
    const manifest = fileManifest(directory); writeFileSync(path.join(directory, 'FILES.json'), JSON.stringify(manifest));
    verifyPayload(directory);
    writeFileSync(path.join(directory, 'docs', 'notice.txt'), 'changed');
    assert.throws(() => verifyPayload(directory), /integrity/);
    writeFileSync(path.join(directory, 'docs', 'notice.txt'), 'notice');
    writeFileSync(path.join(directory, 'extra'), 'unexpected');
    assert.throws(() => verifyPayload(directory), /manifest/);
    rmSync(path.join(directory, 'extra'));
    if (process.platform !== 'win32') {
      symlinkSync(path.join(directory, 'docs', 'notice.txt'), path.join(directory, 'linked'));
      assert.throws(() => verifyPayload(directory), /symlink/);
    }
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('acceptance allows only sanitized owner-approved operation outcomes', async () => {
  const { sanitizeAcceptance } = await import('../lib/acceptance.mjs');
  const catalog = new Set(['vehicle-state']);
  const record = sanitizeAcceptance({ approved: true, model: 'R1T', firmware: '2026.38.1', operations: [{ id: 'vehicle-state', status: 'passed' }] }, catalog);
  assert.deepEqual(record, { approved: true, model: 'R1T', firmware: '2026.38.1', operations: [{ id: 'vehicle-state', status: 'passed' }] });
  assert.throws(() => sanitizeAcceptance({ ...record, vin: 'secret' }, catalog), /fields/);
  assert.throws(() => sanitizeAcceptance({ ...record, firmware: 'token secret' }, catalog), /firmware/);
  assert.throws(() => sanitizeAcceptance({ ...record, operations: [{ id: 'unknown', status: 'passed' }] }, catalog), /operation/);
  assert.throws(() => sanitizeAcceptance({ ...record, approved: false }, catalog), /approved/);
});

test('portable version switch verifies locally before replacing pointer and preserves rollback', async () => {
  const { switchVersion, rollbackVersion } = await import('../lib/delivery.mjs');
  const directory = mkdtempSync(path.join(os.tmpdir(), 'rivian-install-'));
  try {
    const { fileManifest } = await import('../lib/delivery.mjs');
    for (const version of ['1.0.0-a', '1.0.1-b']) {
      const payload = path.join(directory, 'versions', version); mkdirSync(payload, { recursive: true });
      writeFileSync(path.join(payload, 'my-rivian-data'), version);
      writeFileSync(path.join(payload, 'FILES.json'), JSON.stringify(fileManifest(payload)));
    }
    switchVersion(directory, '1.0.0-a'); switchVersion(directory, '1.0.1-b');
    assert.equal(rollbackVersion(directory).current, '1.0.0-a');
    writeFileSync(path.join(directory, 'versions', '1.0.1-b', 'my-rivian-data'), 'tampered');
    assert.throws(() => rollbackVersion(directory), /integrity/);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('portable CLI verifies archive digests, installs new directories, rolls back and removes only its payloads', async () => {
  const { fileManifest, sha256 } = await import('../lib/delivery.mjs');
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'rivian-archive-test-'));
  const installer = fileURLToPath(new URL('../install-portable.mjs', import.meta.url));
  const directory = path.join(temporary, 'installed');
  const invoke = args => spawnSync(process.execPath, [installer, ...args], { encoding: 'utf8', shell: false });
  const archiveFor = revision => {
    const name = `my-rivian-data-1.0.0-${process.platform}-${process.arch}`;
    const staging = path.join(temporary, name);
    rmSync(staging, { recursive: true, force: true }); mkdirSync(staging);
    const executable = process.platform === 'win32' ? 'my-rivian-data.exe' : 'my-rivian-data';
    writeFileSync(path.join(staging, executable), 'test-only non-executable fixture');
    writeFileSync(path.join(staging, 'BUILD.json'), JSON.stringify({ version: '1.0.0', platform: process.platform, architecture: process.arch, revision, executable, sha256: sha256(path.join(staging, executable)) }));
    writeFileSync(path.join(staging, 'FILES.json'), JSON.stringify(fileManifest(staging)));
    const archive = path.join(temporary, `${revision}.tar.gz`);
    execFileSync('tar', ['-czf', archive, '-C', temporary, name]);
    return archive;
  };
  try {
    const first = archiveFor('a'.repeat(40));
    assert.notEqual(invoke(['install', directory, first, 'f'.repeat(64)]).status, 0);
    assert.equal(existsSync(directory), false);
    assert.equal(invoke(['install', directory, first, sha256(first)]).status, 0);
    const second = archiveFor('b'.repeat(40));
    assert.equal(invoke(['install', directory, second, sha256(second)]).status, 0);
    assert.equal(invoke(['rollback', directory]).status, 0);
    const state = JSON.parse(readFileSync(path.join(directory, 'installed.json'), 'utf8'));
    assert.ok(state.current.includes('a'.repeat(12)));
    writeFileSync(path.join(directory, 'unrelated.txt'), 'keep');
    assert.notEqual(invoke(['uninstall', directory]).status, 0);
    assert.equal(readFileSync(path.join(directory, 'unrelated.txt'), 'utf8'), 'keep');
    rmSync(path.join(directory, 'unrelated.txt'));
    assert.equal(invoke(['uninstall', directory]).status, 0);
    assert.equal(existsSync(directory), false);
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});

test('portable archive rejects actual link entries and traversal paths before extraction', { skip: process.platform === 'win32' }, async () => {
  const { sha256 } = await import('../lib/delivery.mjs');
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'rivian-archive-paths-'));
  const installer = fileURLToPath(new URL('../install-portable.mjs', import.meta.url));
  const name = 'my-rivian-data-fixture';
  mkdirSync(path.join(temporary, name));
  writeFileSync(path.join(temporary, name, 'file'), 'fixture');
  const archive = path.join(temporary, 'links.tar.gz');
  const directory = path.join(temporary, 'installed');
  try {
    symlinkSync('../outside', path.join(temporary, name, 'link'));
    execFileSync('tar', ['-czf', archive, '-C', temporary, name]);
    const links = spawnSync(process.execPath, [installer, 'install', directory, archive, sha256(archive)], { encoding: 'utf8', shell: false });
    assert.notEqual(links.status, 0); assert.match(links.stderr, /links/);
    assert.equal(existsSync(directory), false);
    if (process.platform === 'linux') {
      rmSync(path.join(temporary, name, 'link'));
      execFileSync('tar', ['-czf', archive, '--transform', `s,^${name},../${name},`, '-C', temporary, name]);
      const traversal = spawnSync(process.execPath, [installer, 'install', directory, archive, sha256(archive)], { encoding: 'utf8', shell: false });
      assert.notEqual(traversal.status, 0); assert.match(traversal.stderr, /paths/);
      assert.equal(existsSync(directory), false);
    }
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});

test('Windows lifecycle reports native failure codes and terminates a stuck installer tree', { skip: process.platform !== 'win32' }, () => {
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'rivian-windows-lifecycle-'));
  const checker = fileURLToPath(new URL('../../packaging/windows/check-install.ps1', import.meta.url));
  const fixtureName = `installer-fixture-${process.pid}-${path.basename(temporary)}.exe`;
  const fixture = path.join(temporary, fixtureName);
  const marker = path.join(temporary, 'child-pid');
  const source = `using System;
using System.Diagnostics;
using System.IO;
using System.Threading;
public class InstallerFixture {
  public static int Main(string[] args) {
    if (args.Length > 0 && args[0] == "--child") { Thread.Sleep(30000); return 0; }
    if (Environment.GetEnvironmentVariable("RIVIAN_LIFECYCLE_FIXTURE") == "fail") return 73;
    var child = Process.Start(Process.GetCurrentProcess().MainModule.FileName, "--child");
    File.WriteAllText(Environment.GetEnvironmentVariable("RIVIAN_LIFECYCLE_MARKER"), child.Id.ToString());
    Thread.Sleep(30000);
    return 0;
  }
}`;
  const compile = path.join(temporary, 'compile.ps1');
  writeFileSync(compile, `Add-Type -TypeDefinition @'\n${source}\n'@ -OutputAssembly '${fixture.replaceAll("'", "''")}' -OutputType ConsoleApplication\n`);
  const invoke = mode => spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', checker,
    '-Installer', fixture, '-Version', 'lifecycle-fixture', '-Revision', `${process.pid}-${mode}`, '-TimeoutSeconds', '3'], {
    encoding: 'utf8', timeout: 15000,
    env: { ...process.env, RIVIAN_LIFECYCLE_FIXTURE: mode, RIVIAN_LIFECYCLE_MARKER: marker },
  });
  try {
    execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', compile], { timeout: 15000 });
    const failure = invoke('fail');
    assert.equal(failure.error, undefined);
    assert.notEqual(failure.status, 0);
    assert.match(failure.stderr, /Silent installation failed \(exit code 73\)/u);
    const timeout = invoke('hang');
    assert.equal(timeout.error, undefined, 'The checker must enforce its own deadline');
    assert.notEqual(timeout.status, 0);
    assert.match(timeout.stderr, /Silent installation timed out after 3 seconds/u);
    assert.match(timeout.stdout, /Starting: Silent installation/u);
    const childPid = readFileSync(marker, 'utf8');
    assert.throws(() => process.kill(Number(childPid), 0), /ESRCH/u, 'Timeout must terminate installer descendants');
  } finally {
    // Also clean up the fixture when the regression is exercised against old code.
    spawnSync('taskkill.exe', ['/IM', fixtureName, '/T', '/F'], { encoding: 'utf8' });
    rmSync(temporary, { recursive: true, force: true });
  }
});

const makensis = process.env.RIVIAN_MAKENSIS ?? 'C:\\Program Files (x86)\\NSIS\\makensis.exe';
// Actual installation changes the test profile's Start menu/HKCU pointers. Opt in
// only on a disposable Windows profile, as the dedicated CI step does.
test('Windows NSIS verifies its payload, rejects reinstall and removes the installed build', {
  skip: process.platform !== 'win32' || process.env.RIVIAN_NATIVE_INSTALLER_TESTS !== '1',
}, async () => {
  assert.ok(existsSync(makensis), 'The NSIS compiler is required for native installer acceptance');
  const { fileManifest } = await import('../lib/delivery.mjs');
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'rivian-nsis-lifecycle-'));
  const sourceRoot = fileURLToPath(new URL('../../', import.meta.url));
  const staging = path.join(temporary, 'payload');
  const installer = path.join(temporary, 'setup.exe');
  const version = 'installer-fixture';
  const revision = path.basename(temporary);
  const installed = path.join(process.env.LOCALAPPDATA, 'My Rivian Data', 'versions', `${version}-${revision}`);
  const badInstalled = `${installed}-bad`;
  const restrictedEnv = { ...Object.fromEntries(Object.entries(process.env).filter(([key]) => key.toUpperCase() !== 'PSEXECUTIONPOLICYPREFERENCE')), PSExecutionPolicyPreference: 'Restricted' };
  mkdirSync(staging);
  const compile = path.join(temporary, 'compile.ps1');
  writeFileSync(compile, `Add-Type -TypeDefinition @'
using System;
public class VersionFixture {
  public static int Main(string[] args) {
    if (args.Length == 1 && args[0] == "--version") { Console.WriteLine("My Rivian Data ${version}"); return 0; }
    return 64;
  }
}
'@ -OutputAssembly '${path.join(staging, 'my-rivian-data.exe').replaceAll("'", "''")}' -OutputType ConsoleApplication\n`);
  const invokeInstaller = () => spawnSync(installer, ['/S'], { encoding: 'utf8', timeout: 30000, windowsHide: true, env: restrictedEnv });
  const buildInstaller = buildRevision => execFileSync(makensis, [`/DVERSION=${version}`, `/DREVISION=${buildRevision}`, `/DSOURCE=${staging}`, `/DOUTPUT=${installer}`,
    path.join(sourceRoot, 'packaging/windows/installer.nsi')], { timeout: 45000, windowsHide: true });
  try {
    execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', compile], { timeout: 15000 });
    copyFileSync(path.join(sourceRoot, 'LICENSE'), path.join(staging, 'LICENSE'));
    copyFileSync(path.join(sourceRoot, 'packaging/windows/verify.ps1'), path.join(staging, 'verify.ps1'));
    writeFileSync(path.join(staging, 'FILES.json'), JSON.stringify(fileManifest(staging)));
    assert.equal(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
      "$env:PSModulePath = [IO.Path]::Combine($PSHOME, 'Modules'); Get-ExecutionPolicy"], { encoding: 'utf8', timeout: 15000, env: restrictedEnv }).trim(), 'Restricted');
    buildInstaller(revision);
    const first = invokeInstaller();
    assert.equal(first.error, undefined, 'Silent installation must finish without a hidden dialog');
    assert.equal(first.status, 0, 'The real verifier must accept the intact payload');
    assert.equal(execFileSync(path.join(installed, 'my-rivian-data.exe'), ['--version'], { encoding: 'utf8', timeout: 5000 }).trim(), `My Rivian Data ${version}`);
    const duplicate = invokeInstaller();
    assert.equal(duplicate.error, undefined);
    assert.equal(duplicate.status, 10, 'Silent reinstall must reject without a dialog');
    writeFileSync(path.join(staging, 'LICENSE'), 'changed fixture');
    buildInstaller(`${revision}-bad`);
    const rejected = invokeInstaller();
    assert.equal(rejected.error, undefined, 'Integrity rejection must finish without a hidden dialog');
    assert.equal(rejected.status, 11, 'The installer must reject a changed payload even under its process policy');
    assert.equal(existsSync(path.join(badInstalled, 'my-rivian-data.exe')), false);
    writeFileSync(path.join(installed, 'LICENSE'), 'changed fixture');
    const tampered = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', path.join(installed, 'verify.ps1')], { encoding: 'utf8', timeout: 15000 });
    assert.notEqual(tampered.status, 0, 'The native verifier must reject changed files');
    assert.match(tampered.stderr, /Payload integrity check failed/u);
  } finally {
    if (existsSync(path.join(installed, 'Uninstall.exe'))) {
      const removal = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
        `Start-Process -FilePath '${path.join(installed, 'Uninstall.exe').replaceAll("'", "''")}' -ArgumentList '/S' -Wait`], { encoding: 'utf8', timeout: 30000 });
      assert.equal(removal.error, undefined, 'Silent uninstall must finish without a hidden dialog');
      assert.equal(existsSync(installed), false, 'The uninstaller must remove its payload');
    }
    rmSync(badInstalled, { recursive: true, force: true });
    rmSync(path.join(os.tmpdir(), `my-rivian-data-${version}-${revision}-bad-verify.log`), { force: true });
    rmSync(temporary, { recursive: true, force: true });
  }
});
