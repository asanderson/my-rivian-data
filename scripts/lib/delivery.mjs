import { createHash } from 'node:crypto';
import { existsSync, lstatSync, readFileSync, readdirSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export const sha256 = file => createHash('sha256').update(readFileSync(file)).digest('hex');
export function fileManifest(directory) {
  const files = {};
  function walk(relative = '') {
    for (const name of readdirSync(path.join(directory, relative)).sort()) {
      const file = path.posix.join(relative, name);
      if (file === 'FILES.json') continue;
      const stat = lstatSync(path.join(directory, file));
      if (stat.isSymbolicLink()) throw new Error('Payload symlinks are forbidden.');
      if (stat.isDirectory()) walk(file);
      else if (stat.isFile()) files[file] = sha256(path.join(directory, file));
      else throw new Error('Unsupported payload file type.');
    }
  }
  walk(); return files;
}

export function verifyPayload(directory) {
  const expected = JSON.parse(readFileSync(path.join(directory, 'FILES.json'), 'utf8'));
  const actual = fileManifest(directory);
  if (JSON.stringify(Object.keys(expected).sort()) !== JSON.stringify(Object.keys(actual).sort())) throw new Error('Payload manifest file list differs.');
  for (const [file, digest] of Object.entries(expected)) {
    if (!/^[a-f0-9]{64}$/.test(digest) || actual[file] !== digest) throw new Error('Payload integrity check failed.');
  }
}

function versionDirectory(directory, version) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,140}$/.test(version)) throw new Error('Invalid local version.');
  const versions = path.join(directory, 'versions');
  const candidate = path.join(versions, version);
  if (lstatSync(versions).isSymbolicLink() || lstatSync(candidate).isSymbolicLink()) throw new Error('Version symlinks are forbidden.');
  return candidate;
}
export function switchVersion(directory, version) {
  verifyPayload(versionDirectory(directory, version));
  const pointer = path.join(directory, 'installed.json');
  const previous = existsSync(pointer) ? JSON.parse(readFileSync(pointer, 'utf8')).current : null;
  const state = { current: version, previous: previous === version ? null : previous };
  writeFileSync(`${pointer}.tmp`, JSON.stringify(state, null, 2) + '\n', { mode: 0o600 });
  renameSync(`${pointer}.tmp`, pointer); return state;
}
export function rollbackVersion(directory) {
  const state = JSON.parse(readFileSync(path.join(directory, 'installed.json'), 'utf8'));
  if (!state.previous) throw new Error('No previous local version is installed.');
  return switchVersion(directory, state.previous);
}
