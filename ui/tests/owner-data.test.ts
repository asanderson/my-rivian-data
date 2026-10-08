import assert from 'node:assert/strict';
import test from 'node:test';
import { chargeLabel, readChargingHistory, readChargingStatus, readVehicleLocation, readVehicleState, sampleDate } from '../src/owner-data.ts';

const id = 'sample-vehicle';
const status = { vehicle_id: id, state: 'charging', battery_percent: 48, limit_percent: 80, power_kw: 7.2 };
const position = { vehicle_id: id, location: { latitude: 47.05, longitude: -122.05, accuracy_m: 25 } };
const session = { id: 'session-1', started_at: '2025-12-31T21:00:00Z', energy_kwh: 18.5, duration_minutes: 155 };
const history = { vehicle_id: id, sessions: [session], has_more: false };
const vehicle = { id, name: 'Sample', model: 'R1S', model_year: 2025, battery_percent: 48, estimated_range_km: 241.4, odometer_km: 8050.5, locked: true, charging_state: 'charging', temperature_celsius: 19.5, location: position.location, observed_at: '2026-01-01T12:00:00Z', source: 'synthetic-demo' };

test('owner views reject responses for a different selected vehicle', () => {
  assert.throws(() => readVehicleState({ vehicle }, 'another-vehicle'));
  assert.throws(() => readChargingStatus(status, 'another-vehicle'));
  assert.throws(() => readChargingHistory(history, 'another-vehicle'));
  assert.throws(() => readVehicleLocation(position, 'another-vehicle'));
});
test('owner charging readings require bounded numbers and bounded unique sessions', () => {
  assert.deepEqual(readChargingStatus(status, id), status);
  assert.deepEqual(readChargingHistory(history, id), history);
  for (const battery_percent of [-1, 101, Infinity, '48', null]) assert.throws(() => readChargingStatus({ ...status, battery_percent }, id));
  for (const energy_kwh of [-1, NaN, '18.5']) assert.throws(() => readChargingHistory({ ...history, sessions: [{ ...session, energy_kwh }] }, id));
  assert.throws(() => readChargingHistory({ ...history, sessions: [session, session] }, id));
  assert.throws(() => readChargingHistory({ ...history, sessions: Array.from({ length: 101 }, (_, n) => ({ ...session, id: `session-${n}` })) }, id));
  assert.throws(() => readChargingHistory({ ...history, sessions: [{ ...session, started_at: 'yesterday' }] }, id));
});
test('owner state and location reject malformed samples rather than inventing missing readings', () => {
  assert.deepEqual(readVehicleState({ vehicle }, id), vehicle);
  assert.deepEqual(readVehicleLocation(position, id), position);
  assert.throws(() => readVehicleState({ vehicle: { ...vehicle, temperature_celsius: undefined } }, id));
  assert.throws(() => readVehicleState({ vehicle: { ...vehicle, locked: 'true' } }, id));
  assert.throws(() => readVehicleState({ vehicle: { ...vehicle, source: 'live' } }, id));
  assert.throws(() => readVehicleLocation({ ...position, location: { ...position.location, latitude: 100 } }, id));
});
test('sample dates remain absolute UTC and unknown charging states are unavailable', () => {
  assert.equal(sampleDate('2026-01-01T12:00:00Z'), 'Jan 1, 2026, 12:00 PM UTC');
  assert.equal(sampleDate('invalid'), 'Sample date unavailable');
  assert.equal(chargeLabel('charging'), 'Charging');
  assert.equal(chargeLabel('not_charging'), 'Not charging');
  for (const state of ['unrecognized_upstream_state', '__proto__', 'constructor', 'toString']) {
    assert.equal(chargeLabel(state), 'Charging state unavailable');
  }
});
