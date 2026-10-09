import assert from 'node:assert/strict';
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
  // Reach the 64 MiB ceiling, then prove that one additional page is refused.
  // This checks the compiled module's limit rather than trusting its build flags.
  assert.ok(exports.memory instanceof WebAssembly.Memory, 'Module must expose its memory for the cap check');
  const pages = exports.memory.buffer.byteLength / 65536;
  assert.ok(pages <= 1024, 'Initial WASM memory must fit the configured ceiling');
  exports.memory.grow(1024 - pages);
  assert.equal(exports.memory.buffer.byteLength, 64 * 1024 * 1024);
  assert.throws(() => exports.memory.grow(1), RangeError, '64 MiB linker memory limit must be enforced');
  console.log(`Compiled native/WASM parity passed: ${fixture.cases.length} demo fixtures, ${liveFixture.cases.length} live fixtures, both catalogs and 64 MiB memory cap.`);
});
