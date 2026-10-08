import { spawnSync } from 'node:child_process';
import { existsSync, realpathSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const root = fileURLToPath(new URL('../../', import.meta.url));

export function run(command, args, { capture = false } = {}) {
  if (!capture) console.log(`> ${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, {
    cwd: root,
    shell: false,
    windowsHide: true,
    encoding: 'utf8',
    stdio: capture ? ['ignore', 'pipe', 'pipe'] : 'inherit',
  });
  if (result.error) throw new Error(`Unable to run ${command}: ${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(`${command} failed (${result.status ?? result.signal})${capture ? `\n${result.stderr}` : ''}`);
  }
  return result.stdout;
}

// Invoke npm's JavaScript entry point directly. This avoids Windows .cmd shell
// quoting and keeps every subprocess argument separate on all supported hosts.
function npmCli() {
  const candidates = [process.env.RIVIAN_NPM_CLI, process.env.npm_execpath];
  const directories = [path.dirname(process.execPath), ...(process.env.PATH ?? '').split(path.delimiter)];
  for (const directory of directories.filter(Boolean)) {
    candidates.push(path.join(directory, 'node_modules/npm/bin/npm-cli.js'));
    candidates.push(path.resolve(directory, '../lib/node_modules/npm/bin/npm-cli.js'));
    const executable = path.join(directory, 'npm');
    if (existsSync(executable)) {
      const resolved = realpathSync(executable);
      if (path.basename(resolved) === 'npm-cli.js') candidates.push(resolved);
    }
  }
  const found = candidates.find(candidate => candidate && existsSync(candidate) && /npm-cli\.(?:c?js)$/.test(candidate));
  if (!found) throw new Error('Cannot locate npm-cli.js. Install Node.js with npm, or set RIVIAN_NPM_CLI to its npm-cli.js path.');
  return found;
}

export function npm(args) {
  return run(process.execPath, [npmCli(), ...args]);
}

export function main(task) {
  Promise.resolve().then(task).catch(error => {
    console.error(`\n${error.message}`);
    process.exitCode = 1;
  });
}
