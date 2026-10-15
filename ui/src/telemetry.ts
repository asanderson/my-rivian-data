import type { Vehicle } from './domain.ts';
import type { ChargingStatus, VehicleLocation } from './owner-data.ts';

export type ConnectionState = 'stopped' | 'connecting' | 'connected' | 'reconnecting' | 'authentication_required' | 'error';
export interface TireReading { pressure_bar: number | null; status: string | null; validity: 'valid' | 'invalid' | 'unknown'; observed_at: string | null }
export interface TelemetryReadings {
  id: string; source: 'live'; field_observed_at: Record<string, string>;
  battery_percent: number | null; estimated_range_km: number | null; odometer_km: number | null;
  temperature_celsius: number | null; locked: boolean | null; charging_state: string | null;
  limit_percent: number | null; power_kw: number | null; location: Vehicle['location'];
  tires: Partial<Record<'front_left' | 'front_right' | 'rear_left' | 'rear_right', TireReading>>;
}
export interface TelemetrySnapshot {
  state: ConnectionState; vehicle_id: string | null; readings: TelemetryReadings | null;
  raw_topics: Record<string, unknown>[]; error_code: string | null; compatibility: 'unverified';
}
/** Cancellation of an old native generation is distinct from upstream expiry. */
export function telemetryAuthenticationExpired(snapshot: TelemetrySnapshot): boolean {
  return snapshot.state === 'authentication_required' && snapshot.error_code === 'account_auth_required';
}
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const date = (value: unknown): value is string => typeof value === 'string' && value.length <= 64 && /^\d{4}-\d{2}-\d{2}T/.test(value) && Number.isFinite(Date.parse(value));
const bounded = (value: unknown, min: number, max: number): value is number => typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
const invalid = () => new Error('The live connection returned an unreadable snapshot. HTTP readings remain available.');
const number = (value: unknown, min: number, max: number): number | null => {
  if (value === null || value === undefined) return null;
  if (!bounded(value, min, max)) throw invalid();
  return value;
};

/** Read only bounded normalized data belonging to the current selection. */
export function readTelemetrySnapshot(value: unknown, vehicleId: string): TelemetrySnapshot {
  if (!object(value) || !['stopped', 'connecting', 'connected', 'reconnecting', 'authentication_required', 'error'].includes(String(value.state)) ||
      !(value.vehicle_id === null || value.vehicle_id === vehicleId) || value.compatibility !== 'unverified' ||
      !(value.error_code === null || typeof value.error_code === 'string' && /^[a-z_]{1,80}$/.test(value.error_code)) ||
      !Array.isArray(value.raw_topics) || value.raw_topics.length > 64 || !value.raw_topics.every(row => object(row) &&
        typeof row.rvm === 'string' && row.rvm.length > 0 && row.rvm.length <= 128 && typeof row.payload_base64 === 'string' && row.payload_base64.length <= 1024 &&
        Number.isSafeInteger(row.timestamp_ms) && date(row.observed_at) && bounded(row.payload_bytes, 0, 32768) && typeof row.truncated === 'boolean' && row.decoded === false)) throw invalid();
  let readings: TelemetryReadings | null = null;
  if (value.readings !== null && value.readings !== undefined) {
    const row = value.readings;
    if (!object(row) || row.id !== vehicleId || value.vehicle_id !== vehicleId || row.source !== 'live' || !object(row.field_observed_at) || Object.keys(row.field_observed_at).length > 64) throw invalid();
    const times: Record<string, string> = {};
    for (const [key, at] of Object.entries(row.field_observed_at)) {
      if (key.length > 128 || !date(at)) throw invalid();
      times[key] = at;
    }
    if (!(row.locked === undefined || row.locked === null || typeof row.locked === 'boolean') ||
        !(row.charging_state === undefined || row.charging_state === null || ['charging', 'complete', 'plugged_in', 'disconnected', 'unknown', 'not_charging'].includes(String(row.charging_state)))) throw invalid();
    let location: Vehicle['location'] = null;
    if (row.location !== null && row.location !== undefined) {
      if (!object(row.location) || !bounded(row.location.latitude, -90, 90) || !bounded(row.location.longitude, -180, 180)) throw invalid();
      location = { latitude: row.location.latitude, longitude: row.location.longitude, accuracy_m: number(row.location.accuracy_m, 0, 100000) };
    }
    const tires: TelemetryReadings['tires'] = {};
    if (row.tires !== undefined) {
      if (!object(row.tires) || Object.keys(row.tires).length > 4) throw invalid();
      for (const position of ['front_left', 'front_right', 'rear_left', 'rear_right'] as const) {
        const tire = row.tires[position];
        if (tire === undefined) continue;
        if (!object(tire) || !['valid', 'invalid', 'unknown'].includes(String(tire.validity)) ||
            !(tire.observed_at === null || date(tire.observed_at)) || !(tire.status === null || ['normal', 'warning_hard', 'warning_soft', 'warning_puncture'].includes(String(tire.status)))) throw invalid();
        const pressure = number(tire.pressure_bar, 0, 20);
        tires[position] = { pressure_bar: tire.validity === 'valid' && date(tire.observed_at) ? pressure : null,
          status: tire.validity === 'valid' ? tire.status as string | null : null, validity: tire.validity as TireReading['validity'], observed_at: tire.observed_at as string | null };
      }
    }
    readings = { id: vehicleId, source: 'live', field_observed_at: times,
      battery_percent: number(row.battery_percent, 0, 100), estimated_range_km: number(row.estimated_range_km, 0, 5000), odometer_km: number(row.odometer_km, 0, 10000000),
      temperature_celsius: number(row.temperature_celsius, -100, 100), limit_percent: number(row.limit_percent, 0, 100), power_kw: number(row.power_kw, 0, 1000),
      locked: row.locked as boolean | null ?? null, charging_state: row.charging_state as string | null ?? null, location, tires };
  }
  return { state: value.state as ConnectionState, vehicle_id: value.vehicle_id as string | null, readings, raw_topics: value.raw_topics, error_code: value.error_code as string | null, compatibility: 'unverified' };
}

/** Null/absent stream fields never replace HTTP fallback; encoded zero/false do. */
export function mergeTelemetryVehicle(vehicle: Vehicle, readings: TelemetryReadings | null | undefined): Vehicle {
  if (!readings || readings.id !== vehicle.id) return vehicle;
  const merged = { ...vehicle };
  for (const key of ['battery_percent', 'estimated_range_km', 'odometer_km', 'temperature_celsius', 'locked', 'charging_state', 'location'] as const) {
    if (streamFieldWins(key, readings, vehicle.field_observed_at)) Object.assign(merged, { [key]: readings[key], field_observed_at: { ...merged.field_observed_at, [key]: readings.field_observed_at[key] } });
  }
  return merged;
}

export function streamFieldWins(key: keyof TelemetryReadings, readings: TelemetryReadings, httpTimes?: Record<string, string>): boolean {
  const at = readings.field_observed_at[key];
  const previous = httpTimes?.[key];
  return readings[key] !== null && !!at && (!previous || Date.parse(at) > Date.parse(previous));
}

export function mergeTelemetryCharging(status: ChargingStatus, readings: TelemetryReadings): ChargingStatus {
  if (status.vehicle_id !== readings.id) return status;
  const merged = { ...status };
  for (const field of ['battery_percent', 'limit_percent', 'power_kw'] as const) if (streamFieldWins(field, readings, status.field_observed_at)) {
    merged[field] = readings[field]; merged.field_observed_at = { ...merged.field_observed_at, [field]: readings.field_observed_at[field] };
  }
  if (readings.charging_state && streamFieldWins('charging_state', readings, status.field_observed_at)) {
    merged.state = readings.charging_state; merged.field_observed_at = { ...merged.field_observed_at, charging_state: readings.field_observed_at.charging_state };
  }
  if (merged.state !== 'charging') { merged.power_kw = null; if (merged.field_observed_at) { merged.field_observed_at = { ...merged.field_observed_at }; delete merged.field_observed_at.power_kw; } }
  return merged;
}

export function mergeTelemetryLocation(position: VehicleLocation, readings: TelemetryReadings): VehicleLocation {
  return position.vehicle_id === readings.id && streamFieldWins('location', readings, position.field_observed_at)
    ? { ...position, location: readings.location, field_observed_at: { ...position.field_observed_at, location: readings.field_observed_at.location } } : position;
}

interface PollDependencies {
  get: () => Promise<unknown>; schedule: (callback: () => void, delay: number) => number; cancel: (timer: number) => void;
}
/** Sequential polling and a close guard keep late responses out of a newer account/vehicle. */
export function createTelemetryPoller(vehicleId: string, onSnapshot: (snapshot: TelemetrySnapshot) => void, onError: (error: unknown) => void, dependencies: PollDependencies) {
  let closed = false, inFlight = false;
  let timer: number | undefined;
  async function poll() {
    if (closed || inFlight) return;
    inFlight = true;
    try {
      const value = await dependencies.get();
      if (!closed) onSnapshot(readTelemetrySnapshot(value, vehicleId));
    } catch (error) { if (!closed) onError(error); }
    finally {
      inFlight = false;
      if (!closed) timer = dependencies.schedule(() => { timer = undefined; void poll(); }, 2000);
    }
  }
  return { poll, close() { closed = true; if (timer !== undefined) dependencies.cancel(timer); } };
}
