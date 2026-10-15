/**
 * Live-mode browser contract tests. The local host and WASM are real; every Rivian
 * login/OTP/data response below is fake and intercepted inside Playwright. No
 * account credentials or requests are sent to Rivian by this test.
 */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { access, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { chromium } from 'playwright';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const binary = process.env.MY_RIVIAN_DATA_BINARY ?? path.join(root, 'target/release', process.platform === 'win32' ? 'my-rivian-data.exe' : 'my-rivian-data');
await access(binary);
const screenshots = path.join(root, 'artifacts/screenshots');
await mkdir(screenshots, { recursive: true });
let host;
let browser;
let launchSecret = '';
const sanitize = value => String(value)
  .replaceAll(launchSecret || '__no_secret__', '[REDACTED]')
  .replace(/#bootstrap=[^\s"'<>]+/g, '#bootstrap=[REDACTED]');
const capabilityKey = 'my-rivian-data-tab-capability';

async function launchNativeSession() {
  host = spawn(binary, ['--session-only', '--no-open', '--print-launch-url'], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  return new Promise((resolve, reject) => {
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
    host.stderr.on('data', () => { /* Never echo potentially sensitive diagnostics. */ });
  });
}

try {
  // Live mode uses a real local host with credential-store IO disabled.
  const launchUrl = await launchNativeSession();
  const origin = new URL(launchUrl).origin;
  browser = await chromium.launch({
    headless: true,
    ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE } : {}),
    args: process.env.CI ? ['--no-sandbox'] : [],
  });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1080 }, reducedMotion: 'reduce' });
  const page = await context.newPage();
  page.setDefaultTimeout(15_000);
  const errors = [];
  const externalRequests = [];
  page.on('pageerror', error => errors.push(sanitize(error.message)));
  page.on('request', request => { if (new URL(request.url()).origin !== origin) externalRequests.push(request.url()); });
  await page.addInitScript(() => {
    window.__cspViolations = [];
    document.addEventListener('securitypolicyviolation', event => window.__cspViolations.push(event.violatedDirective));
  });
  await page.goto(launchUrl, { waitUntil: 'networkidle' });
  assert.equal(new URL(page.url()).hash, '', 'Bootstrap fragment must be scrubbed.');
  await page.getByRole('heading', { name: 'Connect your Rivian account', exact: true }).waitFor();
  assert.equal(await page.getByText('Offline demo', { exact: true }).count(), 0);
  assert.equal(await page.locator('.owner-data').count(), 0);
  await page.screenshot({ path: path.join(screenshots, 'live-sign-in.png'), fullPage: true });
  const storage = () => page.evaluate(() => ({ local: Object.keys(localStorage), session: Object.keys(sessionStorage) }));
  assert.deepEqual(await storage(), { local: [], session: [capabilityKey] });

  // A new tab receives host-scoped cookies, but no origin-bound tab capability.
  const cookieOnlyPage = await context.newPage();
  await cookieOnlyPage.goto(origin, { waitUntil: 'networkidle' });
  await cookieOnlyPage.getByRole('heading', { name: 'Open a fresh session.', exact: true }).waitFor();
  await cookieOnlyPage.close();

  let accountState = 'signed_out';
  let loginCount = 0;
  let otpCount = 0;
  let releaseLogin;
  let markLogin;
  const loginSeen = new Promise(resolve => { markLogin = resolve; });
  const loginRelease = new Promise(resolve => { releaseLogin = resolve; });
  const fulfill = (route, body, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
  const requireSessionHeaders = request => {
    assert.ok(request.headers()['x-app-capability'], 'Account/data requests need the origin-bound app capability.');
    if (request.method() === 'POST') assert.ok(request.headers()['x-csrf-token'], 'Account/data writes need CSRF protection.');
  };
  let remembering = false;
  const vault = () => ({ backend: 'isolated_test_store', available: true, remembering, stored: remembering, error: null });
  await page.route('**/api/account/vault', route => { requireSessionHeaders(route.request()); return fulfill(route, vault()); });
  await page.route('**/api/account/remember', route => {
    requireSessionHeaders(route.request()); remembering = route.request().postDataJSON().remember;
    return fulfill(route, vault());
  });
  await page.route('**/api/account', route => {
    requireSessionHeaders(route.request());
    return fulfill(route, { state: accountState });
  });
  await page.route('**/api/account/login', async route => {
    requireSessionHeaders(route.request());
    const body = route.request().postDataJSON();
    assert.deepEqual(body, { email: 'browser-test@example.invalid', password: 'FAKE_BROWSER_TEST_PASSWORD', remember: true });
    remembering = body.remember;
    loginCount += 1; markLogin(); await loginRelease;
    accountState = 'mfa_required';
    await fulfill(route, { state: accountState, channel: 'TEST authenticator' });
  });
  let resendCount = 0;
  await page.route('**/api/account/otp/resend', route => {
    requireSessionHeaders(route.request()); assert.deepEqual(route.request().postDataJSON(), {}); resendCount += 1;
    return fulfill(route, { state: 'mfa_required', channel: 'TEST authenticator' });
  });
  await page.route('**/api/account/otp', async route => {
    requireSessionHeaders(route.request());
    assert.deepEqual(route.request().postDataJSON(), { code: '123456' });
    otpCount += 1; accountState = 'authenticated';
    await fulfill(route, { state: accountState });
  });
  await page.route('**/api/account/logout', async route => {
    requireSessionHeaders(route.request()); accountState = 'signed_out'; remembering = false;
    await fulfill(route, { state: accountState });
  });
  const vehicle = {
    id: 'browser-test-vehicle-001', name: 'Contract test R1T', model: 'R1T', model_year: 2025,
    battery_percent: 67, estimated_range_km: null, odometer_km: 12000, locked: null,
    charging_state: null, temperature_celsius: null, location: null, observed_at: null, source: 'live',
  };
  const secondVehicle = { ...vehicle, id: 'browser-test-vehicle-002', name: 'Contract test R1S', model: 'R1S', battery_percent: 55 };
  await page.route('**/api/vehicles', route => {
    requireSessionHeaders(route.request()); return fulfill(route, [vehicle, secondVehicle]);
  });
  let expireNextRead = false;
  await page.route('**/api/execute', async route => {
    requireSessionHeaders(route.request());
    const body = route.request().postDataJSON();
    if (expireNextRead) {
      expireNextRead = false; accountState = 'signed_out';
      return fulfill(route, { error: { code: 'account_auth_required', message: 'Sign in to Rivian again.' } }, 401);
    }
    const selected = [vehicle, secondVehicle].find(item => item.id === body.variables.vehicle_id);
    assert.ok(selected);
    const data = {
      'vehicle-state': { vehicle: selected },
      'charging-status': { vehicle_id: selected.id, state: 'unknown', battery_percent: selected.battery_percent, limit_percent: null, power_kw: null },
      'charging-history': { vehicle_id: selected.id, sessions: [], has_more: false },
      'vehicle-location': { vehicle_id: selected.id, location: null },
    }[body.operation_id];
    assert.ok(data, 'The browser should request only cataloged owner operations.');
    return fulfill(route, { mode: 'live', data: { operation_id: body.operation_id, source: 'rivian', synthetic: false, observed_at: null, received_at: '2026-10-09T12:00:00Z', data } });
  });

  await page.reload({ waitUntil: 'networkidle' });

  await page.getByLabel('Email address', { exact: true }).fill('browser-test@example.invalid');
  await page.getByLabel('Password', { exact: true }).fill('FAKE_BROWSER_TEST_PASSWORD');
  await page.getByRole('checkbox', { name: 'Remember sign-in on this device', exact: true }).check();
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await loginSeen;
  assert.equal(await page.getByLabel('Password', { exact: true }).inputValue(), '', 'Password is cleared while sign-in is pending.');
  assert.deepEqual(await storage(), { local: [], session: [capabilityKey] });
  releaseLogin();
  await page.getByRole('heading', { name: 'Verify your sign-in', exact: true }).waitFor();
  await page.getByRole('button', { name: 'Resend verification code', exact: true }).click();
  await page.getByText('Rivian was asked to send another verification code.', { exact: true }).waitFor();
  assert.equal(resendCount, 1);
  await page.getByLabel('Verification code', { exact: true }).fill('123456');
  await page.getByRole('button', { name: 'Verify and continue', exact: true }).click();
  await page.locator('.owner-data').waitFor();
  assert.equal(loginCount, 1); assert.equal(otpCount, 1);
  assert.match(await page.locator('.owner-data').innerText(), /67/);
  assert.match(await page.locator('.owner-data').innerText(), /unavailable|not available/i);
  assert.match(await page.locator('.owner-data').innerText(), /Rivian has not supplied one observation time/);
  assert.doesNotMatch(await page.locator('.owner-data').innerText(), /Demo|Synthetic|FAKE_BROWSER_TEST_PASSWORD|123456/);
  assert.equal(await page.getByRole('checkbox', { name: 'Remember sign-in on this device', exact: true }).isChecked(), true);
  await page.getByRole('checkbox', { name: 'Remember sign-in on this device', exact: true }).click();
  await page.getByText('Your sign-in is kept in memory unless you choose to remember it on this device.', { exact: true }).waitFor();
  assert.equal(await page.getByRole('checkbox', { name: 'Remember sign-in on this device', exact: true }).isChecked(), false);
  assert.deepEqual(await storage(), { local: [], session: [capabilityKey] });

  // Live polling stays local, accepts explicit zero, and drops old vehicle replies.
  const stream = id => ({ state: 'connected', vehicle_id: id, compatibility: 'unverified', error_code: null, raw_topics: [], readings: { id, source: 'live', battery_percent: id === vehicle.id ? 0 : 23, field_observed_at: { battery_percent: '2026-10-14T12:00:00Z' }, tires: {} } });
  let streamId = vehicle.id;
  let releasePoll, markPoll;
  const pollSeen = new Promise(resolve => { markPoll = resolve; });
  const pollRelease = new Promise(resolve => { releasePoll = resolve; });
  let holdPoll = true;
  let stops = 0;
  await page.route('**/api/telemetry/start', route => { requireSessionHeaders(route.request()); streamId = route.request().postDataJSON().vehicle_id; return fulfill(route, stream(streamId)); });
  await page.route('**/api/telemetry/stop', route => { requireSessionHeaders(route.request()); stops += 1; return fulfill(route, { state: 'stopped', vehicle_id: null, compatibility: 'unverified', error_code: null, raw_topics: [], readings: null }); });
  await page.route('**/api/telemetry', async route => {
    requireSessionHeaders(route.request()); const id = streamId;
    if (holdPoll) { holdPoll = false; markPoll(); await pollRelease; }
    await fulfill(route, stream(id)).catch(() => {});
  });
  await page.getByRole('button', { name: 'Start live updates', exact: true }).click();
  await pollSeen;
  assert.match(await page.locator('.owner-metrics').innerText(), /Battery charge\s+0\s*%/);
  await page.getByLabel('Vehicle', { exact: true }).selectOption(secondVehicle.id);
  await page.locator(`.owner-data[aria-label="${secondVehicle.name} information"]`).waitFor();
  await page.getByText('Connected', { exact: true }).waitFor();
  releasePoll();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  assert.match(await page.locator('.owner-metrics').innerText(), /Battery charge\s+23\s*%/);
  assert.equal(await page.locator(`.owner-data[aria-label="${vehicle.name} information"]`).count(), 0);
  await page.getByRole('button', { name: 'Stop live updates', exact: true }).click();
  await page.getByRole('button', { name: 'Start live updates', exact: true }).waitFor();
  assert.ok(stops >= 1);
  await page.getByLabel('Vehicle', { exact: true }).selectOption(vehicle.id);
  await page.locator(`.owner-data[aria-label="${vehicle.name} information"]`).waitFor();
  const ownerNavigation = page.getByRole('navigation', { name: 'Owner features' });
  for (const area of ['Charging', 'Vehicle health', 'Location']) {
    await ownerNavigation.getByRole('button', { name: area, exact: true }).click();
    await page.locator('.owner-data').waitFor();
    assert.doesNotMatch(await page.locator('.owner-data').innerText(), /NaN|undefined/);
  }
  await page.getByRole('navigation', { name: 'Developer tools' }).getByRole('button', { name: 'API explorer', exact: true }).click();
  await page.getByText('Rust/WASM ready', { exact: true }).waitFor();
  await page.getByText('Upstream GraphQL document', { exact: true }).click();
  assert.match(await page.locator('.developer-request-preview').filter({ hasText: 'Upstream GraphQL document' }).innerText(), /query[\s\S]*vehicleState/);
  await page.getByRole('button', { name: 'Run request', exact: true }).click();
  await page.getByText(/^HTTP 200 · \d+ ms$/).waitFor();
  const raw = JSON.parse(await page.locator('.developer-response-json').innerText());
  assert.equal(raw.mode, 'live'); assert.equal(raw.data.synthetic, false);
  assert.equal(raw.data.data.vehicle.id, vehicle.id);
  assert.equal(raw.data.observed_at, null);
  assert.equal(raw.data.received_at, '2026-10-09T12:00:00Z');
  assert.doesNotMatch(await page.locator('.developer-response-json').innerText(), /app_capability|csrf_token|FAKE_BROWSER_TEST_PASSWORD/);
  // A reviewed write is prepared first, exposes its exact changes, and runs once only after confirmation.
  let commandCount = 0;
  await page.route('**/api/commands/prepare', route => {
    requireSessionHeaders(route.request());
    const input = route.request().postDataJSON();
    assert.equal(input.operation_id, 'rename-vehicle');
    assert.deepEqual(input.variables, { vehicle_id: vehicle.id, name: 'Browser contract name' });
    return fulfill(route, { confirmation_id: 'browser_contract_confirmation_001', operation_id: input.operation_id, variables: input.variables, expires_in_seconds: 120 });
  });
  await page.route('**/api/commands/execute', route => {
    requireSessionHeaders(route.request()); commandCount += 1;
    assert.deepEqual(route.request().postDataJSON(), { confirmation_id: 'browser_contract_confirmation_001' });
    return fulfill(route, { mode: 'live', data: { operation_id: 'rename-vehicle', source: 'rivian', synthetic: false, observed_at: null, received_at: '2026-10-14T12:00:00Z', data: { renamed: true }, raw_response: { data: { setVehicleName: true } } } });
  });
  await page.locator('.developer-operation').filter({ hasText: 'rename-vehicle' }).click();
  await page.getByLabel('Request variables JSON', { exact: true }).fill(JSON.stringify({ vehicle_id: vehicle.id, name: 'Browser contract name' }));
  await page.getByRole('button', { name: 'Prepare vehicle rename', exact: true }).click();
  await page.getByRole('button', { name: 'Confirm vehicle rename', exact: true }).waitFor();
  assert.equal(commandCount, 0);
  assert.match(await page.locator('.developer-confirmation').innerText(), /Browser contract name/);
  await page.getByRole('button', { name: 'Confirm vehicle rename', exact: true }).click();
  await page.getByText(/^HTTP 200 · \d+ ms$/).waitFor();
  assert.equal(commandCount, 1); assert.equal(await page.locator('.developer-confirmation').count(), 0);
  assert.deepEqual(await storage(), { local: [], session: [capabilityKey] });

  await page.getByText('Import a reviewed GraphQL document', { exact: true }).click();
  await page.route('**/api/catalog/admit', route => {
    requireSessionHeaders(route.request()); const input = route.request().postDataJSON();
    assert.deepEqual(Object.keys(input).sort(), ['document', 'endpoint', 'operation_name', 'variables']);
    assert.equal(input.operation_name, 'GetVehicleState');
    return fulfill(route, { admitted: true, operation_id: 'vehicle-state', variables: input.variables, requires_confirmation: false });
  });
  await page.getByLabel('Reviewed endpoint', { exact: true }).fill('https://rivian.com/api/gql/gateway/graphql');
  await page.getByLabel('Reviewed operation name', { exact: true }).fill('GetVehicleState');
  await page.getByLabel('Reviewed GraphQL document', { exact: true }).fill('query GetVehicleState { vehicleState { batteryLevel { value } } }');
  await page.getByLabel('Import variables JSON', { exact: true }).fill(JSON.stringify({ vehicle_id: vehicle.id }));
  await page.getByRole('button', { name: 'Check reviewed document', exact: true }).click();
  await page.getByLabel('Reviewed document admission response', { exact: true }).waitFor();
  assert.match(await page.getByLabel('Reviewed document admission response', { exact: true }).innerText(), /vehicle-state/);
  assert.match(await page.getByLabel('Complete request body', { exact: true }).innerText(), /vehicle-state/);
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  await page.reload({ waitUntil: 'networkidle' });
  await page.locator('.owner-data').waitFor();
  assert.deepEqual(await storage(), { local: [], session: [capabilityKey] });
  await page.getByRole('button', { name: 'Disconnect account', exact: true }).click();
  await page.getByRole('heading', { name: 'Connect your Rivian account', exact: true }).waitFor();
  assert.equal(await page.locator('.owner-data,.developer-response-json').count(), 0);

  // Expired Rivian auth must return to sign-in without losing the local session.
  accountState = 'authenticated';
  await page.reload({ waitUntil: 'networkidle' });
  await page.locator('.owner-data').waitFor();
  expireNextRead = true;
  await ownerNavigation.getByRole('button', { name: 'Location', exact: true }).click();
  await page.getByRole('heading', { name: 'Connect your Rivian account', exact: true }).waitFor();
  assert.deepEqual(await storage(), { local: [], session: [capabilityKey] });
  assert.equal(await page.locator('.owner-data,.developer-response-json').count(), 0);
  await page.getByRole('button', { name: 'End session', exact: true }).click();
  await page.getByRole('heading', { name: 'Your session has ended.', exact: true }).waitFor();
  assert.deepEqual(await storage(), { local: [], session: [] });
  assert.equal((await context.cookies()).length, 0);
  assert.deepEqual(await page.evaluate(() => window.__cspViolations), []);
  assert.deepEqual(externalRequests, [], 'Browser requests stay on the local origin.');
  assert.deepEqual(errors, []);
  // Close app is a separate protected action and stops the native process.
  await page.close();
  await new Promise(resolve => { host.once('exit', resolve); host.kill('SIGTERM'); });
  const quitUrl = await launchNativeSession();
  const quitPage = await context.newPage();
  await quitPage.goto(quitUrl, { waitUntil: 'networkidle' });
  await quitPage.getByRole('heading', { name: 'Connect your Rivian account', exact: true }).waitFor();
  const hostClosed = new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Close app did not stop the native process.')), 10000);
    host.once('exit', () => { clearTimeout(timer); resolve(); });
  });
  await quitPage.getByRole('button', { name: 'Close app', exact: true }).click();
  await quitPage.getByRole('heading', { name: 'The app has closed.', exact: true }).waitFor();
  await hostClosed;
  assert.equal(await quitPage.locator('.owner-data,.developer-response-json').count(), 0);
  assert.deepEqual(await quitPage.evaluate(() => ({ local: Object.keys(localStorage), session: Object.keys(sessionStorage) })), { local: [], session: [] });
  assert.equal((await context.cookies()).length, 0);
  console.log('PASS: native live startup, cookie-only tab rejection, fake-only remembered login/MFA resend, nullable owner data, local stream zero/stale-vehicle guards, explicit rename confirmation, reviewed admission, reload/disconnect/account expiry/end session/close app, mobile width and CSP.');
  console.log('No real Rivian account, login or data was used; upstream compatibility still needs owner acceptance testing.');
} catch (error) {
  console.error(`Live browser smoke failed: ${sanitize(error instanceof Error ? error.message : error)}`);
  process.exitCode = 1;
} finally {
  launchSecret = '';
  if (browser) await browser.close();
  if (host && host.exitCode === null) host.kill('SIGTERM');
}
