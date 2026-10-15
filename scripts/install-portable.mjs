import { spawn } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { main, run } from './lib/process.mjs';
import { rollbackVersion, sha256, switchVersion, verifyPayload } from './lib/delivery.mjs';

main(() => {
  const [action, ...args] = process.argv.slice(2);
  const directory = path.resolve(args[0] ?? path.join(process.platform === 'win32' ? process.env.LOCALAPPDATA ?? os.homedir() : path.join(os.homedir(), '.local/share'), 'my-rivian-data-portable'));
  if (!['install', 'rollback', 'launch', 'uninstall'].includes(action)) throw new Error('Usage: install-portable.mjs install DIRECTORY LOCAL_ARCHIVE EXPECTED_SHA256 | rollback|launch|uninstall DIRECTORY. Quit the app before switching or uninstalling.');
  if (existsSync(directory) && lstatSync(directory).isSymbolicLink()) throw new Error('Install directory cannot be a symlink.');
  if (action === 'install') {
    if (args.length !== 3 || !/^[a-fA-F0-9]{64}$/.test(args[2])) throw new Error('Install requires a local archive and separately verified SHA256 digest.');
    const archive = path.resolve(args[1]);
    if (sha256(archive) !== args[2].toLowerCase()) throw new Error('Archive integrity check failed.');
    const members = run('tar', ['-tzf', archive], { capture: true }).trim().split(/\r?\n/);
    const roots = new Set(members.map(file => file.split('/')[0]));
    if (roots.size !== 1 || !/^my-rivian-data-[A-Za-z0-9._-]+$/.test([...roots][0]) || members.some(file => !/^[A-Za-z0-9_./+@-]+$/.test(file) || file.startsWith('/') || file.split('/').some(part => part === '..'))) throw new Error('Archive paths rejected.');
    if (run('tar', ['-tvzf', archive], { capture: true }).trim().split(/\r?\n/).some(line => !/^[-d]/.test(line))) throw new Error('Archive links and special files rejected.');
    mkdirSync(directory, { recursive: true });
    const versions = path.join(directory, 'versions');
    mkdirSync(versions, { recursive: true });
    if (lstatSync(versions).isSymbolicLink()) throw new Error('Install versions directory cannot be a symlink.');
    const temporary = mkdtempSync(path.join(directory, '.install-'));
    try {
      run('tar', ['-xzf', archive, '-C', temporary]);
      const payload = path.join(temporary, [...roots][0]);
      verifyPayload(payload);
      const build = JSON.parse(readFileSync(path.join(payload, 'BUILD.json'), 'utf8'));
      if (build.platform !== process.platform || build.architecture !== process.arch || !/^[a-f0-9]{40}$/.test(build.revision) || !/^[0-9]+\.[0-9]+\.[0-9]+$/.test(build.version)) throw new Error('Package platform/version does not match this machine.');
      const version = `${build.version}-${build.revision.slice(0, 12)}-${build.platform}-${build.architecture}`;
      const destination = path.join(versions, version);
      if (existsSync(destination)) throw new Error('This exact version is already installed; existing files are never overwritten.');
      renameSync(payload, destination);
      switchVersion(directory, version);
      console.log('Verified local version installed. Previous version retained for rollback.');
    } finally { rmSync(temporary, { recursive: true, force: true }); }
  } else if (action === 'rollback') {
    rollbackVersion(directory); console.log('Previous verified local version selected.');
  } else {
    const state = JSON.parse(readFileSync(path.join(directory, 'installed.json'), 'utf8'));
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,140}$/.test(state.current)) throw new Error('Invalid installation state.');
    const payload = path.join(directory, 'versions', state.current);
    if (lstatSync(path.join(directory, 'versions')).isSymbolicLink() || lstatSync(payload).isSymbolicLink()) throw new Error('Installed version links are forbidden.');
    verifyPayload(payload);
    if (action === 'launch') {
      const executable = path.join(payload, process.platform === 'win32' ? 'my-rivian-data.exe' : 'my-rivian-data');
      const child = spawn(executable, [], { shell: false, stdio: 'inherit' });
      child.on('error', () => { console.error('Unable to launch the local app.'); process.exitCode = 1; });
      child.on('exit', code => { process.exitCode = code ?? 1; });
    } else {
      // Only remove a directory that has a valid app payload and managed pointer.
      if (readdirSync(directory).some(name => !['versions', 'installed.json'].includes(name))) throw new Error('Uninstall refused: unexpected files in the installation directory.');
      for (const version of readdirSync(path.join(directory, 'versions'))) {
        if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,140}$/.test(version) || lstatSync(path.join(directory, 'versions', version)).isSymbolicLink()) throw new Error('Uninstall refused: unexpected version paths.');
        verifyPayload(path.join(directory, 'versions', version));
      }
      rmSync(directory, { recursive: true }); console.log('Portable installation removed.');
    }
  }
});
