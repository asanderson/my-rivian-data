// Check the pinned Rust toolchain against the current stable channel and,
// with --apply, bump every place that spells it out (rust-toolchain.toml, the
// CI workflows, the developer guide). The nightly upstream-toolchain workflow
// runs this and opens a pull request; Cargo, npm and GitHub Actions
// dependencies are covered by Dependabot (.github/dependabot.yml) instead.
//
//   node scripts/upstream-toolchain.mjs                 report
//   node scripts/upstream-toolchain.mjs --apply         rewrite the pins
//   node scripts/upstream-toolchain.mjs --summary FILE  write a Markdown summary (PR body)
//   node scripts/upstream-toolchain.mjs --github-output append changed= to $GITHUB_OUTPUT
import { appendFileSync, writeFileSync } from 'node:fs';
import { main } from './lib/process.mjs';
import { compareVersions, parseStableChannel, pinnedToolchain, rewritePins, stableChannelUrl, staleMentions } from './lib/toolchain-pin.mjs';

function option(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : (process.argv[index + 1] ?? true);
}

function outcome(pinned, stable, changed) {
  if (changed.length) return 'bump';
  const delta = compareVersions(stable, pinned);
  return delta > 0 ? 'bump available' : delta < 0 ? 'pin is newer (kept)' : 'current';
}

function summary({ pinned, stable, changed, stale }) {
  const lines = ['## Rust toolchain check', '', '| Pin | Pinned | Stable channel | Result |', '|---|---|---|---|',
    `| \`rust-toolchain.toml\` channel | \`${pinned}\` | \`${stable}\` | ${outcome(pinned, stable, changed)} |`];
  if (changed.length) {
    lines.push('', '### Bumped in this change', '', ...changed.map(file => `- \`${file}\``), '',
      'The Verify workflow runs the full pinned-toolchain build on this PR: clippy with `-D warnings` is the usual reason a toolchain bump needs source changes.',
      'Cargo dependencies are untouched (`--locked` builds keep using `Cargo.lock`).');
  }
  if (stale.length) {
    lines.push('', '### Still describes the old toolchain (refresh by hand with the new toolchain)', '',
      ...stale.map(file => `- \`${file}\``));
  }
  return `${lines.join('\n')}\n`;
}

main(async () => {
  const pinned = pinnedToolchain();
  const response = await fetch(stableChannelUrl, { headers: { 'User-Agent': 'my-rivian-data upstream-toolchain' } });
  if (!response.ok) throw new Error(`${stableChannelUrl}: HTTP ${response.status}`);
  const stable = parseStableChannel(await response.text());
  const apply = process.argv.includes('--apply');
  let changed = [];
  let stale = [];
  if (compareVersions(stable, pinned) > 0) {
    stale = staleMentions(pinned);
    changed = apply ? rewritePins(stable) : [];
  }
  console.log(`rust toolchain  ${pinned} -> ${stable}  ${changed.length ? `bumped ${changed.join(', ')}` : outcome(pinned, stable, changed)}`);
  const summaryFile = option('--summary');
  if (typeof summaryFile === 'string') writeFileSync(summaryFile, summary({ pinned, stable, changed, stale }));
  if (process.argv.includes('--github-output') && process.env.GITHUB_OUTPUT) {
    appendFileSync(process.env.GITHUB_OUTPUT, `changed=${changed.length ? 'true' : 'false'}\n`);
  }
});
