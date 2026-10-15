import { spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { root } from './process.mjs';

let macKeychain;
// Do not print command arguments or captured diagnostics: they may contain
// certificate passwords or notary credentials. CI supplies these env vars only.
function secretRun(command, args) {
  const result = spawnSync(command, args, { cwd: root, shell: false, windowsHide: true, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (result.error || result.status !== 0) throw new Error(`Signing tool ${path.basename(command)} failed; inspect private CI diagnostics.`);
  return result.stdout;
}
function required(names) {
  for (const name of names) if (!process.env[name]) throw new Error(`Signed packaging requires CI secret ${name}.`);
}
export function prepareSigning() {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'my-rivian-data-signing-'));
  try {
    if (process.platform === 'win32') {
      required(['RIVIAN_WINDOWS_PFX_BASE64', 'RIVIAN_WINDOWS_PFX_PASSWORD', 'RIVIAN_TIMESTAMP_URL']);
      if (!/^https:\/\//.test(process.env.RIVIAN_TIMESTAMP_URL)) throw new Error('Timestamp URL must use HTTPS.');
      process.env.RIVIAN_WINDOWS_PFX_PATH = path.join(directory, 'certificate.pfx');
      writeFileSync(process.env.RIVIAN_WINDOWS_PFX_PATH, Buffer.from(process.env.RIVIAN_WINDOWS_PFX_BASE64, 'base64'), { mode: 0o600 });
      const fingerprint = secretRun('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', path.join(root, 'packaging/windows/import-signing.ps1')]).trim();
      if (!/^[A-Fa-f0-9]{40}$/.test(fingerprint)) throw new Error('No imported code signing certificate fingerprint.');
      process.env.RIVIAN_WINDOWS_CERT_SHA1 = fingerprint;
      // Find SDK SignTool on GitHub-hosted Windows images without assuming PATH.
      if (!process.env.RIVIAN_SIGNTOOL) {
        const sdk = path.join(process.env['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)', 'Windows Kits/10/bin');
        const version = readdirSync(sdk).filter(value => /^10\./.test(value)).sort().at(-1);
        process.env.RIVIAN_SIGNTOOL = path.join(sdk, version ?? '', 'x64/signtool.exe');
      }
    } else if (process.platform === 'darwin') {
      required(['RIVIAN_MAC_P12_BASE64', 'RIVIAN_MAC_P12_PASSWORD', 'RIVIAN_MAC_IDENTITY', 'RIVIAN_APPLE_ID', 'RIVIAN_APPLE_TEAM_ID', 'RIVIAN_APPLE_APP_PASSWORD']);
      if (!process.env.RIVIAN_MAC_IDENTITY.startsWith('Developer ID Application:')) throw new Error('A Developer ID Application signing identity is required.');
      const certificate = path.join(directory, 'certificate.p12');
      writeFileSync(certificate, Buffer.from(process.env.RIVIAN_MAC_P12_BASE64, 'base64'), { mode: 0o600 });
      macKeychain = path.join(directory, 'signing.keychain-db');
      const password = randomBytes(32).toString('hex');
      secretRun('security', ['create-keychain', '-p', password, macKeychain]);
      secretRun('security', ['unlock-keychain', '-p', password, macKeychain]);
      secretRun('security', ['set-keychain-settings', '-lut', '21600', macKeychain]);
      secretRun('security', ['import', certificate, '-k', macKeychain, '-P', process.env.RIVIAN_MAC_P12_PASSWORD, '-T', '/usr/bin/codesign']);
      secretRun('security', ['set-key-partition-list', '-S', 'apple-tool:,apple:,codesign:', '-s', '-k', password, macKeychain]);
      secretRun('xcrun', ['notarytool', 'store-credentials', 'my-rivian-data-release', '--keychain', macKeychain, '--apple-id', process.env.RIVIAN_APPLE_ID, '--team-id', process.env.RIVIAN_APPLE_TEAM_ID, '--password', process.env.RIVIAN_APPLE_APP_PASSWORD]);
    }
  } catch (error) { rmSync(directory, { recursive: true, force: true }); throw error; }
  return () => {
    try {
      if (macKeychain) secretRun('security', ['delete-keychain', macKeychain]);
      if (process.env.RIVIAN_WINDOWS_CERT_SHA1) secretRun('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', 'Remove-Item -LiteralPath ("Cert:\\CurrentUser\\My\\" + $env:RIVIAN_WINDOWS_CERT_SHA1) -Force']);
    } finally { rmSync(directory, { recursive: true, force: true }); }
  };
}
export function signWindows(file) {
  required(['RIVIAN_WINDOWS_CERT_SHA1', 'RIVIAN_SIGNTOOL', 'RIVIAN_TIMESTAMP_URL']);
  secretRun(process.env.RIVIAN_SIGNTOOL, ['sign', '/sha1', process.env.RIVIAN_WINDOWS_CERT_SHA1, '/fd', 'SHA256', '/tr', process.env.RIVIAN_TIMESTAMP_URL, '/td', 'SHA256', file]);
  secretRun(process.env.RIVIAN_SIGNTOOL, ['verify', '/pa', '/all', '/v', file]);
}
export function signMac(file, kind) {
  if (!macKeychain) throw new Error('Signing keychain was not initialized.');
  secretRun('codesign', ['--force', '--sign', process.env.RIVIAN_MAC_IDENTITY, '--keychain', macKeychain, '--timestamp', ...(kind === 'dmg' ? [] : ['--options', 'runtime']), file]);
  secretRun('codesign', ['--verify', '--strict', '--verbose=2', file]);
  if (kind === 'binary') return;
  let submission = file;
  if (kind === 'app') {
    submission = `${file}.zip`;
    secretRun('ditto', ['-c', '-k', '--keepParent', file, submission]);
  }
  try {
    const response = JSON.parse(secretRun('xcrun', ['notarytool', 'submit', submission, '--keychain-profile', 'my-rivian-data-release', '--keychain', macKeychain, '--wait', '--output-format', 'json']));
    if (response.status !== 'Accepted') throw new Error('Notarization was not accepted.');
    secretRun('xcrun', ['stapler', 'staple', file]);
    secretRun('xcrun', ['stapler', 'validate', file]);
    secretRun('spctl', ['--assess', '--type', kind === 'app' ? 'execute' : 'open', ...(kind === 'dmg' ? ['--context', 'context:primary-signature'] : []), file]);
  } finally { if (kind === 'app') rmSync(submission, { force: true }); }
}
