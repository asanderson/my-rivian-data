import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

test('acceptance rejects identity, free text, duplicate outcomes and unapproved records', async () => {
  const { sanitizeAcceptance } = await import('../lib/acceptance.mjs');
  const allowed = new Set(['vehicle-state', 'lock-vehicle']);
  const valid = { approved: true, model: 'R1S', firmware: '2026.38.1', operations: [{ id: 'vehicle-state', status: 'not-tested' }, { id: 'lock-vehicle', status: 'blocked' }] };
  assert.deepEqual(sanitizeAcceptance(valid, allowed), valid);
  for (const mutation of [
    { ...valid, name: 'Owner' }, { ...valid, approved: false }, { ...valid, model: 'R1S owner@example.test' },
    { ...valid, firmware: '2026.38.1 token' }, { ...valid, operations: [{ id: 'vehicle-state', status: 'passed', notes: 'secret' }] },
    { ...valid, operations: [{ id: 'vehicle-state', status: 'passed' }, { id: 'vehicle-state', status: 'failed' }] },
    { ...valid, operations: [{ id: 'vehicle-state', status: 'secret' }] }, { ...valid, operations: [{ id: 'not-cataloged', status: 'passed' }] },
  ]) assert.throws(() => sanitizeAcceptance(mutation, allowed));
});

test('launch parser accepts only exact native IPv4 loopback capability URLs', async () => {
  const { parseLaunchUrl } = await import('../lib/local-host.mjs');
  const token = 'a'.repeat(64);
  assert.deepEqual(parseLaunchUrl(`http://127.0.0.1:12345/#bootstrap=${token}`), { origin: 'http://127.0.0.1:12345', token });
  for (const url of [
    `https://127.0.0.1:12345/#bootstrap=${token}`, `http://example.test:12345/#bootstrap=${token}`,
    `http://localhost:12345/#bootstrap=${token}`, `http://127.0.0.1:12345/extra#bootstrap=${token}`,
    `http://user:pass@127.0.0.1:12345/#bootstrap=${token}`, `http://127.0.0.1:12345/?secret=x#bootstrap=${token}`,
    'http://127.0.0.1:12345/#bootstrap=bad', `http://127.0.0.1:12345/#bootstrap=${token}&extra=secret`,
  ]) assert.throws(() => parseLaunchUrl(url), /launch/i);
});

test('demo session uses cookie, origin, capability and csrf without returning response bodies', async () => {
  const { createDemoSession } = await import('../lib/local-host.mjs');
  const requests = [];
  const token = 'a'.repeat(64);
  const csrf = 'b'.repeat(64);
  const capability = 'c'.repeat(64);
  const secret = 'd'.repeat(64);
  const server = createServer(async (request, response) => {
    let body = '';
    for await (const chunk of request) body += chunk;
    requests.push({ path: request.url, headers: request.headers, body });
    response.setHeader('Content-Type', 'application/json');
    if (request.url === '/api/bootstrap') {
      response.setHeader('Set-Cookie', `my_rivian_data_123_${'e'.repeat(16)}=${secret}; HttpOnly; SameSite=Strict; Path=/`);
      response.end(JSON.stringify({ mode: 'demo', csrf_token: csrf, app_capability: capability }));
    } else response.end(JSON.stringify({ secret: 'response bodies must never reach benchmark output' }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const origin = `http://127.0.0.1:${server.address().port}`;
    const session = await createDemoSession(`${origin}/#bootstrap=${token}`);
    const outcome = await session.executeVehicleState();
    assert.equal(outcome.status, 200);
    assert.equal(typeof outcome.latency_ms, 'number');
    assert.deepEqual(Object.keys(outcome).sort(), ['latency_ms', 'status']);
    assert.deepEqual(JSON.parse(requests[0].body), { token });
    assert.equal(requests[1].headers.origin, origin);
    assert.equal(requests[1].headers.cookie, `my_rivian_data_123_${'e'.repeat(16)}=${secret}`);
    assert.equal(requests[1].headers['x-app-capability'], capability);
    assert.equal(requests[1].headers['x-csrf-token'], csrf);
    assert.deepEqual(JSON.parse(requests[1].body), { operation_id: 'vehicle-state', variables: { vehicle_id: 'demo-r1t-001' } });
    assert.equal(JSON.stringify(session), '{}');
  } finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
});

test('process parsers measure the process only and handle command names containing parentheses', async () => {
  const { parseLinuxProcess, parseMacProcess, parseWindowsProcess } = await import('../lib/local-host.mjs');
  const fields = Array(30).fill('0');
  fields[0] = 'S'; fields[11] = '250'; fields[12] = '150'; fields[13] = '999999'; fields[14] = '999999';
  assert.deepEqual(parseLinuxProcess(`42 (app (worker)) ${fields.join(' ')}`, 'Name:\tprivate\nVmRSS:\t2048 kB\n', 100), { rss_bytes: 2097152, cpu_seconds: 4 });
  assert.deepEqual(parseMacProcess('2048 01:02.50'), { rss_bytes: 2097152, cpu_seconds: 62.5 });
  assert.deepEqual(parseMacProcess('2048 1-01:02:03'), { rss_bytes: 2097152, cpu_seconds: 90123 });
  assert.deepEqual(parseWindowsProcess('{"rss_bytes":2097152,"cpu_seconds":4}'), { rss_bytes: 2097152, cpu_seconds: 4 });
  assert.throws(() => parseWindowsProcess('{"rss_bytes":2097152,"cpu_seconds":4,"secret":"x"}'));
  assert.throws(() => parseMacProcess('private error response'));
});

test('benchmark options enforce an explicit demo mode and bounded resource use', async () => {
  const { benchmarkOptions } = await import('../benchmark.mjs');
  assert.deepEqual(benchmarkOptions(['--demo']), { demo: true, duration_seconds: 10, concurrency: 2, sample_interval_ms: 500 });
  assert.equal(benchmarkOptions(['--demo', '--duration-seconds', '3600']).duration_seconds, 3600);
  for (const args of [[], ['--demo', '--concurrency', '0'], ['--demo', '--duration-seconds', '86401'], ['--demo', '--sample-interval-ms', '99'], ['--demo', '--unknown']]) {
    assert.throws(() => benchmarkOptions(args));
  }
});

test('benchmark produces numeric evidence and excludes native output, launch secrets and response content', { skip: process.platform === 'win32' }, async () => {
  const { runBenchmark } = await import('../benchmark.mjs');
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'rivian-benchmark-'));
  const binary = path.join(temporary, 'test-host');
  const token = 'a'.repeat(64); const csrf = 'b'.repeat(64); const capability = 'c'.repeat(64); const cookieSecret = 'd'.repeat(64);
  const privateText = 'private-native-and-response-sentinel';
  writeFileSync(binary, `#!/usr/bin/env node
import { createServer } from 'node:http';
if (process.argv[2] === '--version') { console.log('My Rivian Data 0.1.0'); process.exit(0); }
if (JSON.stringify(process.argv.slice(2)) !== JSON.stringify(['--demo', '--no-open', '--print-launch-url'])) process.exit(2);
const server = createServer(async (request, response) => {
  for await (const chunk of request) {}
  response.setHeader('Content-Type', 'application/json');
  if (request.url === '/api/health') response.end(JSON.stringify({ status: 'ok', mode: 'demo' }));
  else if (request.url === '/api/bootstrap') {
    response.setHeader('Set-Cookie', 'my_rivian_data_123_${'e'.repeat(16)}=${cookieSecret}; HttpOnly; SameSite=Strict; Path=/');
    response.end(JSON.stringify({ mode: 'demo', csrf_token: '${csrf}', app_capability: '${capability}' }));
  } else response.end(JSON.stringify({ private: '${privateText}' }));
});
server.listen(0, '127.0.0.1', () => {
  console.log('My Rivian Data: sample mode at http://127.0.0.1:' + server.address().port);
  console.log('Launch URL (keep private): http://127.0.0.1:' + server.address().port + '/#bootstrap=${token}');
  console.error('${privateText}');
});
process.on('SIGINT', () => { server.closeAllConnections(); server.close(() => process.exit(0)); });
`, { mode: 0o700 });
  let directory;
  try {
    const result = await runBenchmark({ demo: true, binary, duration_seconds: 1, concurrency: 2, sample_interval_ms: 100 });
    directory = result.directory;
    assert.ok(result.summary.requests.total > 0);
    assert.equal(result.summary.requests.failed, 0);
    assert.ok(result.summary.backend.metric_samples > 0);
    assert.ok(result.summary.backend.rss_peak_bytes > 0);
    assert.equal(result.summary.build.version, '0.1.0');
    assert.equal(result.summary.build.source_revision, 'unknown');
    assert.match(result.summary.build.executable_sha256, /^[a-f0-9]{64}$/u);
    assert.equal(result.summary.backend_exited_early, false);
    const serialized = readFileSync(path.join(directory, 'summary.json'), 'utf8') + readFileSync(path.join(directory, 'raw.json'), 'utf8');
    for (const value of [token, csrf, capability, cookieSecret, privateText, '127.0.0.1', '--print-launch-url', binary]) assert.equal(serialized.includes(value), false);
  } finally {
    if (directory) rmSync(directory, { recursive: true, force: true });
    rmSync(temporary, { recursive: true, force: true });
  }
});
