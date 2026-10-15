import { main, npm, run } from './lib/process.mjs';
import { readdirSync } from 'node:fs';

main(() => {
  run('cargo', ['fmt', '--all', '--', '--check']);
  // The host embeds ui/dist at compile time, so build it before workspace checks.
  run(process.execPath, ['scripts/build.mjs']);
  run('cargo', ['clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']);
  run('cargo', ['test', '--workspace', '--locked']);
  npm(['run', 'typecheck', '--prefix', 'ui']);
  npm(['test', '--prefix', 'ui']);
  const deliveryTests = readdirSync(new URL('./tests/', import.meta.url))
    .filter(name => name.endsWith('.test.mjs')).sort().map(name => `scripts/tests/${name}`);
  if (deliveryTests.length === 0) throw new Error('Delivery regression tests are missing.');
  run(process.execPath, ['--test', ...deliveryTests]);
  run(process.execPath, ['scripts/check-parity.mjs']);
  console.log('\nBuild and verification passed on this host. See docs/STATUS.md for checks on other platforms.');
});
