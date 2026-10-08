import path from 'node:path';
import { existsSync } from 'node:fs';
import { main, npm, root, run } from './lib/process.mjs';
import { requireToolchain } from './lib/toolchain.mjs';

main(() => {
  requireToolchain();
  if (!existsSync(path.join(root, 'ui/node_modules'))) throw new Error('UI dependencies are missing. Run node scripts/setup.mjs first.');
  run('cargo', ['build', '-p', 'rivian-wasm', '--target', 'wasm32-unknown-unknown', '--release', '--locked']);
  run('wasm-bindgen', [
    '--target', 'web', '--out-dir', 'ui/public/wasm', '--out-name', 'rivian_wasm',
    'target/wasm32-unknown-unknown/release/rivian_wasm.wasm',
  ]);
  npm(['run', 'build', '--prefix', 'ui']);
  run('cargo', ['build', '-p', 'rivian-host', '--release', '--locked']);
  const executable = path.join(root, 'target/release', process.platform === 'win32' ? 'rivian-local.exe' : 'rivian-local');
  console.log(`\nSelf-contained application: ${executable}\nRun it to open the offline demo in your browser.`);
});
