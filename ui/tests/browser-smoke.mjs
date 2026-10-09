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
  host = spawn(binary, ['--demo', '--no-open', '--print-launch-url'], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
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
  const ownerNavigation = page.getByRole('navigation', { name: 'Owner features' });
  const ownerArea = name => ownerNavigation.getByRole('button', { name, exact: true }).click();
  const ownerData = name => page.getByLabel(`${name} sample information`, { exact: true });
  const checkWidth = async label => assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), true, `${label} must have no horizontal overflow.`);
  await page.getByRole('heading', { name: 'Your vehicles. Your data.', exact: true }).waitFor();
  await ownerData('Demo R1T').waitFor();
  assert.equal(await page.locator('.developer-view').count(), 0, 'Raw API tools must not appear in the owner workspace.');
  assert.equal(await page.getByRole('textbox').count(), 0, 'Owners should not have to enter JSON.');
  assert.doesNotMatch(await page.locator('.owner-view').innerText(), /WASM|JSON|GraphQL|API|validator/i);
  assert.match(await ownerData('Demo R1T').innerText(), /72/);
  const wasmLoaded = await page.evaluate(() => performance.getEntriesByType('resource').some(entry => entry.name.includes('.wasm')));
  assert.equal(wasmLoaded, true, 'Owner features must use the real compiled validator.');
  assert.deepEqual(await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length })), { local: 0, session: 1 });
  await checkWidth('Desktop owner overview');
  await page.screenshot({ path: path.join(screenshots, 'desktop.png'), fullPage: true });

  await ownerArea('Charging');
  await page.getByRole('heading', { name: 'Charging history', exact: true }).waitFor();
  assert.equal(await page.locator('.owner-sessions li').count(), 2);
  assert.match(await page.locator('.owner-history-summary').innerText(), /43\.5/);
  assert.match(await page.locator('.owner-status').innerText(), /Not plugged in/);
  await page.getByLabel('Vehicle', { exact: true }).selectOption('demo-r1s-002');
  await ownerData('Demo R1S').waitFor();
  assert.match(await page.locator('.owner-status').innerText(), /Charging/);
  assert.match(await page.locator('.owner-metrics').innerText(), /7\.2/);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(screenshots, 'charging.png'), fullPage: true });

  // Hold an older vehicle response until the new vehicle is visible.
  let delayedRoute;
  let markDelayed;
  const delayed = new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('Expected delayed charging request was not received.')), 15_000);
    markDelayed = () => { clearTimeout(timeout); resolve(); };
  });
  const delayedHandler = async route => {
    const input = route.request().postDataJSON();
    if (input.operation_id === 'charging-history' && input.variables.vehicle_id === 'demo-r1t-001' && !delayedRoute) {
      delayedRoute = { route, response: await route.fetch() };
      markDelayed();
    } else await route.continue();
  };
  await page.route('**/api/execute', delayedHandler);
  await page.getByLabel('Vehicle', { exact: true }).selectOption('demo-r1t-001');
  await delayed;
  assert.equal(await page.locator('.owner-data').count(), 0, 'The prior vehicle must disappear while loading.');
  await page.getByLabel('Vehicle', { exact: true }).selectOption('demo-r1s-002');
  await ownerData('Demo R1S').waitFor();
  await delayedRoute.route.fulfill({ response: delayedRoute.response });
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  assert.equal(await ownerData('Demo R1T').count(), 0, 'Late data must not be attributed to the new vehicle.');
  assert.match(await page.locator('.owner-status').innerText(), /Charging/);
  await page.unroute('**/api/execute', delayedHandler);

  // A failed owner request has a plain-language recovery path.
  let failedOnce = false;
  const failureHandler = async route => {
    const input = route.request().postDataJSON();
    if (input.operation_id === 'charging-history' && !failedOnce) {
      failedOnce = true;
      await route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: { code: 'unavailable', message: 'Test service unavailable.' } }) });
    } else await route.continue();
  };
  await page.route('**/api/execute', failureHandler);
  await page.getByLabel('Vehicle', { exact: true }).selectOption('demo-r1t-001');
  await page.getByRole('heading', { name: 'We could not load this sample', exact: true }).waitFor();
  await page.getByRole('button', { name: 'Try again', exact: true }).click();
  await ownerData('Demo R1T').waitFor();
  await page.unroute('**/api/execute', failureHandler);

  await ownerArea('Vehicle health');
  await page.getByRole('heading', { name: 'Available readings', exact: true }).waitFor();
  assert.match(await page.locator('.owner-data').innerText(), /12,840/);
  assert.match(await page.locator('.owner-data').innerText(), /does not identify a sensor/);
  await page.getByRole('heading', { name: 'Battery health', exact: true }).waitFor();
  assert.match(await page.locator('.owner-unavailable-grid').innerText(), /battery condition and degradation are not/);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(screenshots, 'vehicle-health.png'), fullPage: true });
  await ownerArea('Location');
  await page.getByRole('heading', { name: 'Location details', exact: true }).waitFor();
  assert.match(await page.locator('.owner-location-values').innerText(), /47\.0000/);
  await page.getByText('No map or real location is loaded.', { exact: true }).waitFor();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(screenshots, 'location.png'), fullPage: true });

  // Desktop developer workspace exposes local wire bodies and response status.
  await page.getByRole('navigation', { name: 'Developer tools' }).getByRole('button', { name: 'API explorer', exact: true }).click();
  await page.getByRole('heading', { name: 'API explorer', exact: true }).waitFor();
  await page.getByText('Rust/WASM ready', { exact: true }).waitFor();
  assert.equal(await page.locator('.owner-view').count(), 0);
  const preview = JSON.parse(await page.getByLabel('Complete request body', { exact: true }).innerText());
  assert.equal(preview.operation_id, 'vehicle-state');
  assert.equal(preview.variables.vehicle_id, 'demo-r1t-001');
  await page.getByRole('button', { name: 'Validate inputs', exact: true }).click();
  await page.getByText('Both validators accept these inputs.', { exact: true }).waitFor();
  // Capture the real native response before Chromium releases its protocol body.
  // No fixture is substituted: route.fetch sends the browser's actual request to
  // the local host and fulfill forwards that same status, headers and body.
  let rawWireBody;
  await page.route('**/api/execute', async route => {
    assert.equal(route.request().postDataJSON().operation_id, 'vehicle-state');
    const response = await route.fetch();
    rawWireBody = await response.text();
    await route.fulfill({ response });
  }, { times: 1 });
  await page.getByRole('button', { name: 'Run demo request', exact: true }).click();
  await page.getByText(/^HTTP 200 · \d+ ms$/).waitFor();
  assert.equal(typeof rawWireBody, 'string', 'Capture the native response before comparing the developer view.');
  const rawResponse = page.getByLabel('Demo vehicle state raw response', { exact: true });
  const result = JSON.parse(await rawResponse.innerText());
  assert.equal(result.mode, 'demo');
  assert.equal(result.data.synthetic, true);
  assert.equal(result.data.data.vehicle.id, 'demo-r1t-001');
  assert.doesNotMatch(await rawResponse.innerText(), /csrf|bootstrap|set-cookie/i);
  await page.getByRole('checkbox', { name: 'Format JSON' }).uncheck();
  assert.equal(await rawResponse.innerText(), rawWireBody, 'Raw response must match the actual bounded HTTP body.');
  await page.getByRole('checkbox', { name: 'Format JSON' }).check();
  await checkWidth('Desktop developer view');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(screenshots, 'developer.png'), fullPage: true });

  await page.getByLabel('Request variables JSON', { exact: true }).fill('{"vehicle_id":"unrecognized-demo-id"}');
  await page.getByRole('button', { name: 'Validate inputs', exact: true }).click();
  await page.getByText('One or both validators rejected these inputs.', { exact: true }).waitFor();
  assert.equal(await page.locator('.developer-validation-issues li').count() >= 2, true);
  const blocked = page.locator('.developer-operation').filter({ hasText: 'unlock-vehicle' });
  await blocked.click();
  assert.equal(await page.getByRole('button', { name: 'Run demo request', exact: true }).isDisabled(), true);
  await page.getByText(/Execution is disabled for this vehicle command/).waitFor();
  await page.getByLabel('Search API requests').fill('charging-history');
  assert.equal(await page.locator('.developer-operation').count(), 1);
  await page.locator('.developer-operation').click();
  assert.match(await page.locator('.developer-parameters').innerText(), /1–100/);
  await page.getByLabel('Search API requests').fill('');
  await page.getByLabel('Vehicle', { exact: true }).selectOption('demo-r1s-002');
  assert.match(await page.getByLabel('Complete request body', { exact: true }).innerText(), /demo-r1s-002/);
  assert.equal(await page.locator('.developer-response-json').count(), 0, 'Vehicle changes clear previous responses.');

  // Mobile owner/developer views stay usable and do not leak technical details into owner screens.
  await page.setViewportSize({ width: 390, height: 844 });
  await ownerArea('Overview');
  await ownerData('Demo R1S').waitFor();
  await checkWidth('Mobile owner overview');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(screenshots, 'mobile.png'), fullPage: true });
  for (const name of ['Charging', 'Vehicle health', 'Location']) {
    await ownerArea(name); await ownerData('Demo R1S').waitFor(); await checkWidth(`Mobile ${name}`);
  }
  await page.getByRole('navigation', { name: 'Developer tools' }).getByRole('button', { name: 'API explorer', exact: true }).click();
  await page.getByText('Rust/WASM ready', { exact: true }).waitFor();
  await page.getByRole('button', { name: 'Run demo request', exact: true }).click();
  await page.getByText(/^HTTP 200 · \d+ ms$/).waitFor();
  await checkWidth('Mobile developer view');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(screenshots, 'developer-mobile.png'), fullPage: true });

  assert.deepEqual(await page.evaluate(() => window.__cspViolations), [], 'Views must not violate CSP.');
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('heading', { name: 'Your vehicles. Your data.', exact: true }).waitFor();
  await ownerData('Demo R1T').waitFor();
  await page.getByRole('button', { name: 'End session', exact: true }).click();
  await page.getByRole('heading', { name: 'Your session has ended.', exact: true }).waitFor();
  assert.equal(await page.locator('.owner-data,.developer-response-json').count(), 0, 'Logout removes all displayed vehicle data.');
  assert.equal(await context.cookies().then(cookies => cookies.length), 0, 'Logout must clear the session cookie.');
  assert.deepEqual(await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length })), { local: 0, session: 0 });
  assert.deepEqual(await page.evaluate(() => window.__cspViolations), [], 'The page must not violate CSP.');
  // An intentionally simulated HTTP 503 is expected; actual script/resource errors are not.
  assert.deepEqual(browserErrors.filter(message => !message.includes('503 (Service Unavailable)')), [], 'Unexpected browser errors.');
  console.log('PASS: owner feature views, vehicle switching/stale replies/retry, developer raw request+response, validation, blocked controls, reload/logout, CSP, desktop/mobile layout.');
  console.log('Token-free screenshots: artifacts/screenshots/{desktop,mobile,charging,vehicle-health,location,developer,developer-mobile}.png');
} catch (error) {
  console.error(`Browser smoke failed: ${sanitize(error instanceof Error ? error.message : error)}`);
  process.exitCode = 1;
} finally {
  launchSecret = '';
  if (browser) await browser.close();
  if (host && host.exitCode === null) host.kill('SIGTERM');
}
