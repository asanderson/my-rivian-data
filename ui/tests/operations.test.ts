import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { runDemoQuery } from '../src/operations.ts';
import type { WasmModule } from '../src/domain.ts';

Object.defineProperty(globalThis, 'window', { value: globalThis, configurable: true });
const realFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = realFetch; });
const valid = { valid: true, issues: [] };
const wasm: WasmModule = { default: async () => undefined, validate_request_json: () => JSON.stringify(valid) };
const demo = { mode: 'demo', data: { operation_id: 'charging-status', source: 'synthetic-demo', synthetic: true, observed_at: '2026-01-01T12:00:00Z', data: { vehicle_id: 'demo-r1t-001', battery_percent: 72 } } };

test('owner queries reject validator disagreement before dispatching execution', async () => {
  const paths: unknown[] = [];
  globalThis.fetch = async path => {
    paths.push(path);
    return new Response(JSON.stringify({ valid: false, issues: [{ field: 'vehicle_id', message: 'Unavailable' }] }));
  };
  await assert.rejects(runDemoQuery(wasm, 'charging-status', { vehicle_id: 'demo-r1t-001' }), /could not check/);
  assert.deepEqual(paths, ['/api/validate']);
});

test('owner queries require a matching operation and explicit synthetic provenance', async () => {
  for (const envelope of [
    { ...demo, mode: 'live' },
    { ...demo, data: { ...demo.data, operation_id: 'vehicle-state' } },
    { ...demo, data: { ...demo.data, synthetic: false } },
    { ...demo, data: { ...demo.data, source: 'unverified' } },
    { ...demo, data: { ...demo.data, observed_at: 'not a date' } },
  ]) {
    globalThis.fetch = async path => new Response(JSON.stringify(path === '/api/validate' ? valid : envelope));
    await assert.rejects(runDemoQuery(wasm, 'charging-status', { vehicle_id: 'demo-r1t-001' }), /sample format/);
  }
});

test('owner queries retain the fixed sample timestamp alongside their data', async () => {
  globalThis.fetch = async path => new Response(JSON.stringify(path === '/api/validate' ? valid : demo));
  assert.deepEqual(await runDemoQuery(wasm, 'charging-status', { vehicle_id: 'demo-r1t-001' }), {
    data: demo.data.data, observedAt: demo.data.observed_at,
  });
});
