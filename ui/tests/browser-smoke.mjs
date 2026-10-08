import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { chromium } from 'playwright';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const binary = process.env.MY_RIVIAN_DATA_BINARY ?? path.join(root, 'target/release', process.platform === 'win32' ? 'my-rivian-data.exe' : 'my-rivian-data');
const screenshots = path.join(root, 'artifacts/screenshots');
await access(binary);
await mkdir(screenshots, { recursive: true });

let host;
let browser;
let launchSecret = '';
const sanitize = value => String(value)
  .replaceAll(launchSecret || '__no_secret__', '[REDACTED]')
  .replace(/#bootstrap=[^\s"'<>]+/g, '#bootstrap=[REDACTED]');

try {
  // This explicit diagnostic flag is scoped to a child process. Its stdout is never logged.
  host = spawn(binary, ['--no-open', '--print-launch-url'], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  const launchUrl = await new Promise((resolve, reject) => {
    let buffer = '';
    const timeout = setTimeout(() => reject(new Error('Local host did not provide a launch URL within 15 seconds.')), 15_000);
    host.stdout.on('data', chunk => {
      buffer += chunk.toString();
      if (buffer.length > 16_384) { clearTimeout(timeout); reject(new Error('Unexpected host startup output.')); return; }
      const match = buffer.match(/http:\/\/127\.0\.0\.1:\d+\/#bootstrap=([A-Za-z0-9_-]+)/);
      if (match) { launchSecret = match[1]; buffer = ''; clearTimeout(timeout); resolve(match[0]); }
    });
    host.once('error', () => { clearTimeout(timeout); reject(new Error('Could not start the local host.')); });
    host.once('exit', () => { clearTimeout(timeout); reject(new Error('The local host exited before the browser opened.')); });
    host.stderr.on('data', () => { /* Never echo host diagnostics that could contain sensitive data. */ });
  });
  browser = await chromium.launch({
    headless: true,
    ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE } : {}),
    args: process.env.CI ? ['--no-sandbox'] : [],
  });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1080 }, reducedMotion: 'reduce' });
  const page = await context.newPage();
  page.setDefaultTimeout(15_000);
  const browserErrors = [];
  page.on('pageerror', error => browserErrors.push(sanitize(error.message)));
  page.on('console', message => { if (message.type() === 'error') browserErrors.push(sanitize(message.text())); });
  await page.addInitScript(() => {
    window.__cspViolations = [];
    document.addEventListener('securitypolicyviolation', event => {
      window.__cspViolations.push(`${event.violatedDirective}: ${event.blockedURI}`);
    });
  });
  await page.goto(launchUrl, { waitUntil: 'networkidle' });
  assert.equal(new URL(page.url()).hash, '', 'The launcher fragment must be removed immediately.');
  await page.getByRole('heading', { name: 'Your vehicles. Your data.' }).waitFor();
  await page.getByText('Rust/WASM ready', { exact: true }).waitFor();
  const wasmLoaded = await page.evaluate(() => performance.getEntriesByType('resource').some(entry => entry.name.includes('.wasm')));
  assert.equal(wasmLoaded, true, 'The compiled WebAssembly file must actually load.');
  assert.deepEqual(await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length })), { local: 0, session: 0 });

  await page.getByRole('button', { name: 'Validate inputs', exact: true }).click();
  await page.getByText('Native and WASM results match', { exact: true }).waitFor();
  await page.getByText('Both validators accept these inputs.', { exact: true }).waitFor();
  await page.getByRole('button', { name: 'Run demo request', exact: true }).click();
  await page.getByRole('status').filter({ hasText: 'completed with a synthetic response' }).waitFor();
  assert.match(await page.locator('.response-json').innerText(), /synthetic-demo/);
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), true, 'Desktop must have no horizontal overflow.');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(screenshots, 'desktop.png'), fullPage: true });

  await page.getByLabel('Request variables', { exact: true }).fill('{"vehicle_id":"unrecognized-demo-id"}');
  await page.getByRole('button', { name: 'Validate inputs', exact: true }).click();
  await page.getByText('One or both validators rejected these inputs.', { exact: true }).waitFor();
  await page.getByText('Native and WASM results match', { exact: true }).waitFor();
  assert.equal(await page.locator('.validation-issues li').count() >= 2, true);

  const blocked = page.locator('.operation-item').filter({ has: page.locator('.status-blocked') }).first();
  await blocked.click();
  await page.getByText('This vehicle control is blocked in the prototype. No command can be sent.', { exact: true }).waitFor();
  assert.equal(await page.getByRole('button', { name: 'Run demo request', exact: true }).isDisabled(), true);

  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator('.operation-item').filter({ hasText: 'Vehicle state' }).click();
  await page.getByRole('button', { name: 'Run demo request', exact: true }).click();
  await page.getByRole('status').filter({ hasText: 'completed with a synthetic response' }).waitFor();
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), true, 'Mobile must have no horizontal overflow.');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(screenshots, 'mobile.png'), fullPage: true });

  assert.deepEqual(await page.evaluate(() => window.__cspViolations), [], 'The initial page must not violate CSP.');
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByText('Rust/WASM ready', { exact: true }).waitFor();
  await page.getByRole('button', { name: 'End session' }).click();
  await page.getByRole('heading', { name: 'Your session has ended.' }).waitFor();
  assert.equal(await context.cookies().then(cookies => cookies.length), 0, 'Logout must clear the session cookie.');
  assert.deepEqual(await page.evaluate(() => window.__cspViolations), [], 'The page must not violate CSP.');
  assert.deepEqual(browserErrors, [], 'The browser must not emit runtime or resource errors.');
  console.log('PASS: native/WASM browser flow, invalid inputs, blocked controls, reload/logout, CSP, desktop/mobile layout.');
  console.log('Token-free screenshots: artifacts/screenshots/desktop.png and mobile.png');
} catch (error) {
  console.error(`Browser smoke failed: ${sanitize(error instanceof Error ? error.message : error)}`);
  process.exitCode = 1;
} finally {
  launchSecret = '';
  if (browser) await browser.close();
  if (host && host.exitCode === null) host.kill('SIGTERM');
}
