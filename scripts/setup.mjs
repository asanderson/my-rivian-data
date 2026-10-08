import { main, npm, run } from './lib/process.mjs';
import { bindgenVersion, hasMatchingBindgen } from './lib/toolchain.mjs';

main(() => {
  if (Number(process.versions.node.split('.')[0]) < 24) throw new Error('Install Node.js 24 or newer, including npm.');
  // rust-toolchain.toml tells rustup which compiler, components and target to use.
  run('cargo', ['fetch', '--locked']);
  if (!hasMatchingBindgen()) {
    run('cargo', ['install', 'wasm-bindgen-cli', '--version', bindgenVersion(), '--locked']);
  }
  npm(['ci', '--prefix', 'ui']);
  console.log('\nDependencies installed. Next: node scripts/build.mjs');
});
