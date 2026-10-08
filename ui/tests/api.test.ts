import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { ApiError, clearSession, request, startSession } from '../src/api.ts';

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
