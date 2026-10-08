import { main, npm, run } from './lib/process.mjs';

main(() => {
  run('cargo', ['fmt', '--all', '--', '--check']);
  // The host embeds ui/dist at compile time, so build it before workspace checks.
  run(process.execPath, ['scripts/build.mjs']);
  run('cargo', ['clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']);
  run('cargo', ['test', '--workspace', '--locked']);
  npm(['run', 'typecheck', '--prefix', 'ui']);
  npm(['test', '--prefix', 'ui']);
  run(process.execPath, ['scripts/check-parity.mjs']);
  console.log('\nBuild and verification passed on this host. See docs/STATUS.md for checks on other platforms.');
});
