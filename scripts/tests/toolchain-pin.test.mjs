import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

const manifest = `manifest-version = "2"
date = "2026-10-13"
[pkg.cargo]
version = "1.99.1 (abc 2026-10-01)"
[pkg.rust]
version = "1.99.1 (58cf8f9c3 2026-10-13)"
git_commit_hash = "58cf8f9c3"
[pkg.rust.target.x86_64-unknown-linux-gnu]
available = true
`;

function fixtureRoot() {
  const dir = mkdtempSync(path.join(tmpdir(), 'toolchain-pin-'));
  mkdirSync(path.join(dir, '.github/workflows'), { recursive: true });
  mkdirSync(path.join(dir, 'docs/dependency-licenses/rust-1.99.0'), { recursive: true });
  writeFileSync(path.join(dir, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.99.0"\nprofile = "minimal"\n');
  writeFileSync(path.join(dir, '.github/workflows/verify.yml'), 'run: rustup toolchain install 1.99.0 --profile minimal\n');
  writeFileSync(path.join(dir, '.github/workflows/release.yml'), '- run: rustup toolchain install 1.99.0 --profile minimal\n- run: rustup toolchain install 1.99.0 --profile minimal\n');
  writeFileSync(path.join(dir, 'docs/DEVELOPER-GUIDE.md'), 'Rust is pinned to 1.99.0, with rustfmt.\n');
  writeFileSync(path.join(dir, 'docs/STATUS.md'), 'Linux checks use the pinned Rust 1.99.0 toolchain.\n');
  writeFileSync(path.join(dir, 'docs/THIRD-PARTY-NOTICES.md'), 'notices for 1.99.0\n');
  return dir;
}

test('stable channel manifest parsing reads the [pkg.rust] version only', async () => {
  const { parseStableChannel, compareVersions } = await import('../lib/toolchain-pin.mjs');
  assert.equal(parseStableChannel(manifest), '1.99.1');
  assert.throws(() => parseStableChannel('[pkg.cargo]\nversion = "1.99.1"\n'), /stable Rust version/);
  assert.equal(compareVersions('1.100.0', '1.99.1') > 0, true);
  assert.equal(compareVersions('1.99.1', '1.99.1'), 0);
});

test('a toolchain bump rewrites every pin together and reports evidence that still names the old one', async () => {
  const { pinnedToolchain, rewritePins, staleMentions, pinFiles } = await import('../lib/toolchain-pin.mjs');
  const dir = fixtureRoot();
  assert.equal(pinnedToolchain(dir), '1.99.0');
  const changed = rewritePins('1.99.1', dir);
  assert.deepEqual(changed, pinFiles.map(entry => entry.file));
  assert.equal(pinnedToolchain(dir), '1.99.1');
  assert.equal(readFileSync(path.join(dir, '.github/workflows/release.yml'), 'utf8').match(/1\.99\.1/gu).length, 2);
  assert.equal(readFileSync(path.join(dir, 'docs/DEVELOPER-GUIDE.md'), 'utf8'), 'Rust is pinned to 1.99.1, with rustfmt.\n');
  assert.deepEqual(rewritePins('1.99.1', dir), []);
  assert.deepEqual(staleMentions('1.99.0', dir), ['docs/STATUS.md', 'docs/THIRD-PARTY-NOTICES.md', 'docs/dependency-licenses/rust-1.99.0/']);
  writeFileSync(path.join(dir, 'rust-toolchain.toml'), '[toolchain]\nchannel = "stable"\n');
  assert.throws(() => rewritePins('1.99.2', dir), /rust-toolchain.toml: toolchain pin not found/);
});

test('the real repository pins one exact toolchain everywhere', async () => {
  const { pinnedToolchain, pinFiles } = await import('../lib/toolchain-pin.mjs');
  const { root } = await import('../lib/process.mjs');
  const version = pinnedToolchain(root);
  for (const { file, pattern } of pinFiles) {
    const text = readFileSync(path.join(root, file), 'utf8');
    pattern.lastIndex = 0;
    const versions = new Set([...text.matchAll(new RegExp(pattern.source, pattern.flags.includes('g') ? pattern.flags : `${pattern.flags}g`))].map(match => match[2]));
    assert.deepEqual([...versions], [version], `${file} pins ${[...versions].join(', ')} but rust-toolchain.toml pins ${version}`);
  }
});
