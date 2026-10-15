import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { main, root, run } from './lib/process.mjs';

main(async () => {
  const fixture = JSON.parse(readFileSync(path.join(root, 'fixtures/validation-cases.json'), 'utf8'));
  const native = JSON.parse(run('cargo', ['run', '--quiet', '--locked', '-p', 'rivian-core', '--example', 'validation_fixture'], { capture: true }));
  const wasm = await import(pathToFileURL(path.join(root, 'ui/public/wasm/rivian_wasm.js')).href);
  const exports = await wasm.default({ module_or_path: readFileSync(path.join(root, 'ui/public/wasm/rivian_wasm_bg.wasm')) });
  assert.equal(fixture.schema_version, 1);
  assert.equal(native.cases.length, fixture.cases.length);
  assert.equal(new Set(fixture.cases.map(item => item.name)).size, fixture.cases.length, 'Fixture names must be unique');
  for (const [index, item] of fixture.cases.entries()) {
    const result = JSON.parse(wasm.validate_request_json(item.operation_id, item.variables_json));
    assert.equal(native.cases[index].name, item.name, 'Native fixture ordering');
    assert.deepEqual(result, native.cases[index].result, `Native/WASM parity: ${item.name}`);
    assert.equal(result.valid, item.expected_valid, `Expected validity: ${item.name}`);
  }
  assert.deepEqual(JSON.parse(wasm.catalog_json()), native.catalog, 'Native/WASM catalog parity');
  const liveFixture = JSON.parse(readFileSync(path.join(root, 'fixtures/live-validation-cases.json'), 'utf8'));
  assert.equal(liveFixture.schema_version, 1);
  assert.equal(native.live_cases.length, liveFixture.cases.length);
  for (const [index, item] of liveFixture.cases.entries()) {
    const result = JSON.parse(wasm.validate_live_request_json(item.operation_id, item.variables_json));
    assert.equal(native.live_cases[index].name, item.name, 'Native live fixture ordering');
    assert.deepEqual(result, native.live_cases[index].result, `Native/WASM live parity: ${item.name}`);
    assert.equal(result.valid, item.expected_valid, `Expected live validity: ${item.name}`);
  }
  assert.deepEqual(JSON.parse(wasm.live_catalog_json()), native.live_catalog, 'Native/WASM live catalog parity');
  // The complete reviewed inventory plus enriched rows exceed spawnSync's
  // default 1 MiB capture limit. Keep this reference capture explicitly bounded.
  const reference = spawnSync('cargo', ['run', '--quiet', '--locked', '-p', 'rivian-core', '--example', 'parallax_fixture'], {
    cwd: root, encoding: 'utf8', shell: false, windowsHide: true, maxBuffer: 4 * 1024 * 1024,
  });
  if (reference.error) throw new Error(`Unable to run native telemetry fixture: ${reference.error.message}`);
  assert.equal(reference.status, 0, `Native telemetry fixture failed: ${reference.stderr}`);
  const telemetry = JSON.parse(reference.stdout);
  assert.deepEqual(JSON.parse(wasm.inventory_json()), telemetry.inventory, 'Native/WASM reviewed inventory parity');
  assert.deepEqual(JSON.parse(wasm.enriched_live_catalog_json()), telemetry.enriched_live_catalog, 'Native/WASM enriched catalog parity');
  assert.equal(telemetry.inventory.entries.length, 234, 'Reviewed source inventory must remain complete');
  assert.equal(telemetry.enriched_live_catalog.length, 244, 'Enriched catalog must retain all local/source rows');
  assert.equal(new Set(telemetry.cases.map(item => item.name)).size, telemetry.cases.length, 'Telemetry fixture names must be unique');
  const decoded = new Map();
  const errors = new Set();
  for (const item of telemetry.cases) {
    const result = JSON.parse(wasm.decode_parallax_fixture_json(item.input_json));
    assert.deepEqual(result, item.result, `Native/WASM telemetry parity: ${item.name}`);
    assert.equal(result.valid, result.error === null && result.outcomes.every(outcome => !outcome.error), `Telemetry validity: ${item.name}`);
    if (result.error) errors.add(result.error);
    for (const outcome of result.outcomes) if (outcome.error) errors.add(outcome.error);
    decoded.set(item.name, result);
  }
  for (const error of ['input_too_large', 'too_many_messages', 'malformed_json', 'invalid_input',
    'invalid_vehicle', 'invalid_topic', 'invalid_timestamp', 'payload_too_large', 'malformed_payload',
    'invalid_reading', 'stale_message', 'resource_limit']) {
    assert.ok(errors.has(error), `Compiled parity must cover rejection: ${error}`);
  }
  const readings = JSON.parse(readFileSync(path.join(root, 'fixtures/parallax-readings.json'), 'utf8'));
  for (const [field, expected] of Object.entries(readings.expected)) {
    assert.deepEqual(decoded.get('documented-owner-readings').snapshot[field], expected, `Documented telemetry: ${field}`);
  }
  assert.equal(decoded.get('explicit-zero').snapshot.estimated_range_km, 0, 'Explicit zero must remain a reading');
  assert.deepEqual(decoded.get('empty-frame-preserves-original-age').snapshot,
    decoded.get('documented-owner-readings').snapshot, 'Empty frames must preserve original readings and ages');
  assert.equal(decoded.get('rejected-frame-does-not-advance-time').snapshot.estimated_range_km, 0, 'Rejected frame must permit same-timestamp retry');
  const limited = decoded.get('unknown-topic-resource-limit-preserves-known-topic');
  assert.equal(limited.developer_entries.length, 16, 'Unknown telemetry history must remain bounded');
  assert.equal(limited.snapshot.estimated_range_km, 315, 'Unknown topic limit must preserve known-topic decoding');
  const truncated = decoded.get('unknown-developer-payload-is-truncated').developer_entries[0];
  assert.equal(truncated.truncated, true);
  assert.equal(truncated.payload_base64.length, 1024, 'Unknown telemetry payload display must remain bounded');
  // Reach the 64 MiB ceiling, then prove that one additional page is refused.
  // This checks the compiled module's limit rather than trusting its build flags.
  assert.ok(exports.memory instanceof WebAssembly.Memory, 'Module must expose its memory for the cap check');
  const pages = exports.memory.buffer.byteLength / 65536;
  assert.ok(pages <= 1024, 'Initial WASM memory must fit the configured ceiling');
  exports.memory.grow(1024 - pages);
  assert.equal(exports.memory.buffer.byteLength, 64 * 1024 * 1024);
  assert.throws(() => exports.memory.grow(1), RangeError, '64 MiB linker memory limit must be enforced');
  console.log(`Compiled native/WASM parity passed: ${fixture.cases.length} demo fixtures, ${liveFixture.cases.length} live fixtures, ${telemetry.cases.length} telemetry/error fixtures, catalogs, reviewed inventory and 64 MiB memory cap.`);
});
