import assert from 'node:assert/strict';
import test from 'node:test';
import { createTelemetryPoller, mergeTelemetryCharging, mergeTelemetryLocation, mergeTelemetryVehicle, readTelemetrySnapshot, telemetryAuthenticationExpired } from '../src/telemetry.ts';

const at = '2026-10-14T12:00:00Z';
const vehicle = { id: 'vehicle-a', name: 'R1T', model: null, model_year: null, battery_percent: 67, estimated_range_km: null, odometer_km: null, locked: true, charging_state: null, temperature_celsius: null, location: null, observed_at: null, source: 'live' };
const snapshot = { state: 'connected', vehicle_id: vehicle.id, compatibility: 'unverified', error_code: null, raw_topics: [], readings: { id: vehicle.id, source: 'live', battery_percent: 0, locked: false, field_observed_at: { battery_percent: at, locked: at }, tires: { front_left: { pressure_bar: 0, status: 'normal', validity: 'valid', observed_at: at } } } };

test('live snapshots require selected vehicle association and validated provenance', () => {
  const reading = readTelemetrySnapshot(snapshot, vehicle.id);
  assert.equal(mergeTelemetryVehicle(vehicle, reading.readings).battery_percent, 0);
  assert.equal(mergeTelemetryVehicle(vehicle, reading.readings).locked, false);
  assert.throws(() => readTelemetrySnapshot(snapshot, 'vehicle-b'));
  assert.throws(() => readTelemetrySnapshot({ ...snapshot, readings: { ...snapshot.readings, id: 'vehicle-b' } }, vehicle.id));
  for (const readings of [{ ...snapshot.readings, source: 'demo' }, { ...snapshot.readings, battery_percent: -1 }, { ...snapshot.readings, field_observed_at: { battery_percent: 'yesterday' } }]) {
    assert.throws(() => readTelemetrySnapshot({ ...snapshot, readings }, vehicle.id));
  }
});
test('missing or unaged stream fields retain the HTTP fallback and invalid tires stay absent', () => {
  const reading = readTelemetrySnapshot({ ...snapshot, readings: { ...snapshot.readings, battery_percent: null, locked: null, tires: { front_left: { pressure_bar: 3, validity: 'invalid', status: null, observed_at: at } } } }, vehicle.id);
  assert.equal(mergeTelemetryVehicle(vehicle, reading.readings).battery_percent, 67);
  assert.equal(mergeTelemetryVehicle(vehicle, reading.readings).locked, true);
  assert.equal(reading.readings?.tires.front_left?.pressure_bar, null);
  assert.equal(mergeTelemetryVehicle(vehicle, readTelemetrySnapshot({ ...snapshot, readings: { ...snapshot.readings, field_observed_at: {} } }, vehicle.id).readings).battery_percent, 67);
});
test('older stream fields never replace a newer HTTP observation', () => {
  const readings = readTelemetrySnapshot(snapshot, vehicle.id).readings;
  const fresh = { ...vehicle, field_observed_at: { battery_percent: '2026-10-14T12:05:00Z', locked: '2026-10-14T12:05:00Z' } };
  assert.equal(mergeTelemetryVehicle(fresh, readings).battery_percent, 67);
  assert.equal(mergeTelemetryVehicle(fresh, readings).locked, true);
  const old = { ...fresh, field_observed_at: { battery_percent: '2026-10-14T11:00:00Z' } };
  assert.equal(mergeTelemetryVehicle(old, readings).battery_percent, 0);
});
test('newer HTTP charging and location reports survive old stream values and power cannot contradict disconnected state', () => {
  const readings = readTelemetrySnapshot({ ...snapshot, readings: { ...snapshot.readings, charging_state: 'charging', power_kw: 11, limit_percent: 80, location: { latitude: 0, longitude: 0, accuracy_m: null }, field_observed_at: { charging_state: at, power_kw: at, limit_percent: at, location: at } } }, vehicle.id).readings!;
  const times = { charging_state: '2026-10-14T12:05:00Z', limit_percent: '2026-10-14T12:05:00Z', location: '2026-10-14T12:05:00Z' };
  const status = { vehicle_id: vehicle.id, state: 'disconnected', battery_percent: null, limit_percent: 90, power_kw: null, field_observed_at: times };
  assert.equal(mergeTelemetryCharging(status, readings).state, 'disconnected');
  assert.equal(mergeTelemetryCharging(status, readings).limit_percent, 90);
  assert.equal(mergeTelemetryCharging(status, readings).power_kw, null);
  const position = { vehicle_id: vehicle.id, location: { latitude: 47, longitude: -122, accuracy_m: null }, field_observed_at: times };
  assert.deepEqual(mergeTelemetryLocation(position, readings).location, position.location);
  assert.deepEqual(mergeTelemetryLocation({ ...position, field_observed_at: {} }, readings).location, readings.location);
});
test('malformed and oversized raw topic rows cannot enter the developer snapshot', () => {
  for (const raw_topics of [[null], Array.from({ length: 65 }, () => ({})), [{ rvm: 'topic', payload_base64: 'X'.repeat(4097), timestamp_ms: 0, observed_at: at, payload_bytes: 1, truncated: false, decoded: false }]]) {
    assert.throws(() => readTelemetrySnapshot({ ...snapshot, raw_topics }, vehicle.id));
  }
});
test('revoking an old native stream does not expire the current account', () => {
  const revoked = readTelemetrySnapshot({ ...snapshot, state: 'authentication_required', error_code: 'session_revoked', vehicle_id: null, readings: null }, vehicle.id);
  assert.equal(telemetryAuthenticationExpired(revoked), false);
  assert.equal(telemetryAuthenticationExpired({ ...revoked, error_code: 'account_auth_required' }), true);
  assert.equal(telemetryAuthenticationExpired({ ...revoked, error_code: null }), false);
});
test('a stopped native snapshot clears stream readings and preserves the HTTP fallback', () => {
  const stopped = readTelemetrySnapshot({ ...snapshot, state: 'stopped', vehicle_id: null, readings: null }, vehicle.id);
  assert.equal(stopped.readings, null);
  assert.deepEqual(mergeTelemetryVehicle(vehicle, stopped.readings), vehicle);
});
test('poller permits one in-flight request and drops responses after close', async () => {
  let release!: (value: unknown) => void;
  let calls = 0;
  let scheduled: (() => void) | undefined;
  const seen: unknown[] = [];
  const poller = createTelemetryPoller(vehicle.id, value => seen.push(value), () => assert.fail('unexpected poll error'), {
    get: () => { calls++; return new Promise(resolve => { release = resolve; }); },
    schedule: callback => { scheduled = callback; return 1; }, cancel: () => { scheduled = undefined; },
  });
  poller.poll(); poller.poll();
  assert.equal(calls, 1);
  poller.close(); release(snapshot);
  await Promise.resolve(); await Promise.resolve();
  assert.deepEqual(seen, []); assert.equal(scheduled, undefined);
});
test('poller validates a late response before scheduling another local read', async () => {
  const seen: unknown[] = [];
  let delay = 0;
  const poller = createTelemetryPoller(vehicle.id, value => seen.push(value), () => assert.fail('unexpected poll error'), {
    get: async () => snapshot, schedule: (_callback, wait) => { delay = wait; return 1; }, cancel: () => {},
  });
  await poller.poll();
  assert.equal(seen.length, 1); assert.equal(delay, 2000); poller.close();
});
