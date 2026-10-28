import assert from 'node:assert/strict';
import { test } from 'node:test';

test('DMG creation retries a busy hdiutil a bounded number of times and then gives up', async () => {
  const { createDiskImage } = await import('../lib/native-packages.mjs');
  const calls = [];
  const flaky = (failures) => (command, args) => {
    calls.push([command, ...args]);
    if (calls.length <= failures) throw new Error('hdiutil failed (1)');
    return '';
  };
  createDiskImage('/tmp/image-root', '/tmp/out.dmg', { attempts: 3, delayMs: 1, runner: flaky(2) });
  assert.equal(calls.length, 3);
  assert.deepEqual(calls[0], ['hdiutil', 'create', '-ov', '-nospotlight', '-format', 'UDZO', '-volname', 'My Rivian Data', '-srcfolder', '/tmp/image-root', '/tmp/out.dmg']);
  assert.deepEqual(calls[2], calls[0], 'every attempt runs the identical command');

  calls.length = 0;
  assert.throws(() => createDiskImage('/tmp/image-root', '/tmp/out.dmg', { attempts: 3, delayMs: 1, runner: flaky(99) }), /hdiutil failed/);
  assert.equal(calls.length, 3, 'the last failure is surfaced, not swallowed');

  calls.length = 0;
  createDiskImage('/tmp/image-root', '/tmp/out.dmg', { attempts: 3, delayMs: 1, runner: flaky(0) });
  assert.equal(calls.length, 1, 'no retry on success');
});
