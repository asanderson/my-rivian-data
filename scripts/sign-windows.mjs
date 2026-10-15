import { main } from './lib/process.mjs';
import { signWindows } from './lib/signing.mjs';
main(() => {
  if (process.platform !== 'win32' || process.argv.length !== 3) throw new Error('Signing requires one local Windows artifact.');
  signWindows(process.argv[2]);
});
