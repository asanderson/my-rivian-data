import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { root } from './process.mjs';

// The pinned Rust toolchain is spelled out in these files; every one must
// carry the same version, so a bump rewrites them all together.
export const pinFiles = [
  { file: 'rust-toolchain.toml', pattern: /^(channel = ")(\d+\.\d+\.\d+)(")/mu },
  { file: '.github/workflows/verify.yml', pattern: /(rustup toolchain install )(\d+\.\d+\.\d+)( )/gu },
  { file: '.github/workflows/release.yml', pattern: /(rustup toolchain install )(\d+\.\d+\.\d+)( )/gu },
  { file: 'docs/DEVELOPER-GUIDE.md', pattern: /(Rust is pinned to )(\d+\.\d+\.\d+)(,)/u },
];

// Files that mention the pinned version but describe evidence or notices
// gathered WITH that toolchain; they are reported, never rewritten.
export const manualMentions = ['docs/STATUS.md', 'docs/THIRD-PARTY-NOTICES.md', 'docs/dependency-licenses/'];

export const stableChannelUrl = 'https://static.rust-lang.org/dist/channel-rust-stable.toml';

export function pinnedToolchain(projectRoot = root) {
  const toml = readFileSync(path.join(projectRoot, 'rust-toolchain.toml'), 'utf8');
  const version = toml.match(/^channel = "(\d+\.\d+\.\d+)"/mu)?.[1];
  if (!version) throw new Error('rust-toolchain.toml does not pin an exact stable version.');
  return version;
}

// channel-rust-stable.toml: the [pkg.rust] table's version reads like
// `1.99.0 (58cf8f9c3 2026-10-13)`.
export function parseStableChannel(manifest) {
  const table = manifest.match(/^\[pkg\.rust\]\r?\n([\s\S]*?)(?=^\[)/mu)?.[1];
  const version = table?.match(/^version = "(\d+\.\d+\.\d+)(?: \([^)]*\))?"/mu)?.[1];
  if (!version) throw new Error('Cannot read the stable Rust version from the channel manifest.');
  return version;
}

export function compareVersions(a, b) {
  const [pa, pb] = [a, b].map(v => v.split('.').map(Number));
  for (let i = 0; i < 3; i += 1) if (pa[i] !== pb[i]) return pa[i] - pb[i];
  return 0;
}

// Rewrite every pin to `version`; returns the files that changed. Throws when
// a file no longer carries the pattern, so a silent partial bump cannot happen.
export function rewritePins(version, projectRoot = root) {
  const changed = [];
  for (const { file, pattern } of pinFiles) {
    const target = path.join(projectRoot, file);
    const before = readFileSync(target, 'utf8');
    pattern.lastIndex = 0;
    if (!pattern.test(before)) throw new Error(`${file}: toolchain pin not found.`);
    pattern.lastIndex = 0;
    const after = before.replace(pattern, `$1${version}$3`);
    if (after !== before) {
      writeFileSync(target, after);
      changed.push(file);
    }
  }
  return changed;
}

// Evidence/notice locations that still carry the OLD version after a bump.
export function staleMentions(oldVersion, projectRoot = root) {
  const hits = [];
  for (const entry of manualMentions) {
    const target = path.join(projectRoot, entry);
    if (!existsSync(target)) continue;
    if (statSync(target).isDirectory()) {
      if (readdirSync(target).includes(`rust-${oldVersion}`)) hits.push(`${entry}rust-${oldVersion}/`);
    } else if (readFileSync(target, 'utf8').includes(oldVersion)) {
      hits.push(entry);
    }
  }
  return hits;
}
