import assert from 'node:assert/strict';
import test from 'node:test';
import { parseVariables, safeSourceUrl, isValidationResult, validationsMatch } from '../src/domain.ts';

test('only JSON objects cross the request boundary', () => {
  for (const input of ['null', '[]', '3', '"text"', '{broken']) assert.throws(() => parseVariables(input));
  assert.deepEqual(parseVariables('{"vehicle_id":"demo-r1t-001"}'), { vehicle_id: 'demo-r1t-001' });
});
test('the bound uses UTF-8 bytes rather than character count', () => {
  assert.throws(() => parseVariables(JSON.stringify({ x: '😀'.repeat(4096) })), /16 KiB/);
  assert.doesNotThrow(() => parseVariables(JSON.stringify({ x: 'a'.repeat(16_000) })));
});
test('validator parity ignores issue order but detects any semantic difference', () => {
  const a = { valid: false, issues: [{ field: 'b', message: 'B' }, { field: 'a', message: 'A' }] };
  assert.equal(validationsMatch(a, { valid: false, issues: [...a.issues].reverse() }), true);
  assert.equal(validationsMatch(a, { valid: true, issues: a.issues }), false);
  assert.equal(validationsMatch(a, { valid: false, issues: [{ field: 'b', message: 'Different' }] }), false);
});
test('WASM output must have the declared validation shape', () => {
  assert.equal(isValidationResult({ valid: true, issues: [] }), true);
  assert.equal(isValidationResult({ valid: 'true', issues: [] }), false);
  assert.equal(isValidationResult({ valid: false, issues: [{ field: 'x' }] }), false);
});
test('source links cannot use executable or unencrypted URL schemes', () => {
  for (const input of ['javascript:alert(1)', 'http://example.com', '/relative', 'data:text/html,hi']) assert.equal(safeSourceUrl(input), undefined);
  assert.equal(safeSourceUrl('https://example.com/docs'), 'https://example.com/docs');
});
