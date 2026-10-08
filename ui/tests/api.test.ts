import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { ApiError, clearSession, request, requestWithDetails, startSession } from '../src/api.ts';

Object.defineProperty(globalThis, 'window', { value: globalThis, configurable: true });
const realFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = realFetch; clearSession(); });

test('bootstrap stays in the request body; later writes receive the in-memory CSRF token', async () => {
  const calls: { path: unknown; options: RequestInit | undefined }[] = [];
  globalThis.fetch = async (path, options) => {
    calls.push({ path, options });
    return new Response(JSON.stringify(path === '/api/bootstrap'
      ? { csrf_token: 'test-csrf', mode: 'demo' }
      : { ok: true }), { headers: { 'Content-Type': 'application/json' } });
  };
  await startSession('test-bootstrap');
  await request('/api/catalog');
  await request('/api/validate', { operation_id: 'account-summary', variables: {} });
  assert.equal(calls[0].path, '/api/bootstrap');
  assert.equal(calls[0].options?.body, '{"token":"test-bootstrap"}');
  assert.equal(new Headers(calls[0].options?.headers).has('X-CSRF-Token'), false);
  assert.equal(calls[1].options?.method, 'GET');
  assert.equal(calls[1].options?.body, undefined);
  assert.equal(new Headers(calls[1].options?.headers).has('X-CSRF-Token'), false);
  assert.equal(new Headers(calls[2].options?.headers).get('X-CSRF-Token'), 'test-csrf');
  for (const { options } of calls) {
    assert.equal(options?.credentials, 'same-origin');
    assert.equal(options?.redirect, 'error');
    assert.equal(options?.cache, 'no-store');
  }
});

test('session clearing drops the CSRF token and reload uses the session endpoint', async () => {
  let lastHeaders: Headers | undefined;
  let lastPath: unknown;
  globalThis.fetch = async (path, options) => {
    lastPath = path;
    lastHeaders = new Headers(options?.headers);
    return new Response(JSON.stringify({ csrf_token: 'test-csrf', mode: 'demo' }));
  };
  await startSession(null);
  assert.equal(lastPath, '/api/session');
  clearSession();
  await request('/api/logout', {});
  assert.equal(lastHeaders?.has('X-CSRF-Token'), false);
});

test('oversized streamed responses are cancelled before JSON parsing', async () => {
  let cancelled = false;
  globalThis.fetch = async () => new Response(new ReadableStream({
    start(controller) { controller.enqueue(new Uint8Array(1_048_577)); },
    cancel() { cancelled = true; },
  }));
  await assert.rejects(request('/api/catalog'), /larger than the demo limit/);
  assert.equal(cancelled, true);
});

test('HTTP authentication errors preserve their status for the relaunch flow', async () => {
  globalThis.fetch = async () => new Response(JSON.stringify({ error: { code: 'unauthorized', message: 'Session expired.' } }), { status: 401 });
  await assert.rejects(request('/api/session'), error => error instanceof ApiError && error.status === 401 && error.code === 'unauthorized');
});

test('bootstrap rejects a service outside the explicitly supported demo mode', async () => {
  globalThis.fetch = async () => new Response(JSON.stringify({ csrf_token: 'test-csrf', mode: 'live' }));
  await assert.rejects(startSession('test-bootstrap'), /offline demo session/);
});

test('developer requests retain the complete wire body and HTTP metadata without response headers', async () => {
  const raw = '{ "mode": "demo", "data": { "battery_percent": 78 } }';
  let options: RequestInit | undefined;
  globalThis.fetch = async (_path, init) => {
    options = init;
    return new Response(raw, { status: 200, headers: { 'X-Private-Example': 'not-for-display' } });
  };
  const response = await requestWithDetails('/api/execute', { operation_id: 'vehicle-state', variables: {} });
  assert.equal(response.rawBody, raw);
  assert.equal(response.status, 200);
  assert.deepEqual(response.body, { mode: 'demo', data: { battery_percent: 78 } });
  assert.ok(Number.isFinite(response.durationMs) && response.durationMs >= 0);
  assert.deepEqual(Object.keys(response).sort(), ['body', 'durationMs', 'rawBody', 'status']);
  assert.equal(options?.redirect, 'error');
  assert.equal(options?.credentials, 'same-origin');
  assert.equal(options?.cache, 'no-store');
});

test('raw HTTP error details are opt-in and preserve the actual envelope and status', async () => {
  const raw = '{"error":{"code":"operation_disabled","message":"Disabled in demo."}}';
  globalThis.fetch = async () => new Response(raw, { status: 403 });
  await assert.rejects(requestWithDetails('/api/execute', {}), error => {
    assert.ok(error instanceof ApiError);
    assert.equal(error.status, 403);
    assert.equal(error.code, 'operation_disabled');
    assert.equal(error.response?.rawBody, raw);
    assert.equal(error.response?.status, 403);
    assert.ok(Number.isFinite(error.response?.durationMs));
    return true;
  });
  await assert.rejects(request('/api/execute', {}), error => error instanceof ApiError && error.response === undefined);
});

test('developer inspection cannot bypass the streamed response limit for an HTTP error', async () => {
  let cancelled = false;
  globalThis.fetch = async () => new Response(new ReadableStream({
    start(controller) { controller.enqueue(new Uint8Array(1_048_577)); },
    cancel() { cancelled = true; },
  }), { status: 500 });
  await assert.rejects(requestWithDetails('/api/execute', {}), /larger than the demo limit/);
  assert.equal(cancelled, true);
});

test('empty successful responses retain status metadata without parsing a body', async () => {
  globalThis.fetch = async () => new Response(null, { status: 204 });
  const response = await requestWithDetails('/api/logout', {});
  assert.equal(response.body, undefined);
  assert.equal(response.rawBody, '');
  assert.equal(response.status, 204);
  assert.equal(await request('/api/logout', {}), undefined);
});
