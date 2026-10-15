import type { AppMode } from './api.ts';
import type { Vehicle } from './domain.ts';

export interface ChargingStatus {
  vehicle_id: string;
  state: string;
  battery_percent: number | null;
  limit_percent: number | null;
  power_kw: number | null;
  field_observed_at?: Record<string, string>;
}
export interface ChargingSession {
  id: string;
  started_at: string | null;
  energy_kwh: number | null;
  duration_minutes: number | null;
}
export interface ChargingHistory {
  vehicle_id: string;
  sessions: ChargingSession[];
  has_more: boolean;
  unassigned_count?: number;
}
export interface VehicleLocation { vehicle_id: string; location: Vehicle['location']; field_observed_at?: Record<string, string> }

const invalid = () => new Error('This information could not be displayed. Please try again.');
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const bounded = (value: unknown, min: number, max: number): value is number => typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
const text = (value: unknown): value is string => typeof value === 'string' && value.length > 0 && value.length <= 256;
const timestamp = (value: unknown): value is string => text(value) && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(value) && Number.isFinite(Date.parse(value));
const allowedMissing = (value: unknown, mode: AppMode) => value === null && mode === 'live';
const numberField = (value: unknown, min: number, max: number, mode: AppMode) => bounded(value, min, max) || allowedMissing(value, mode);
const location = (value: unknown, mode: AppMode): value is Vehicle['location'] => allowedMissing(value, mode) || record(value) && bounded(value.latitude, -90, 90) && bounded(value.longitude, -180, 180) && numberField(value.accuracy_m, 0, 100_000, mode);

/** Native HTTP timestamps use upstream field names; retain and normalize their source ages. */
function fieldTimes(value: unknown): Record<string, string> | undefined {
  if (value === undefined) return undefined;
  if (!record(value) || Object.keys(value).length > 64) throw invalid();
  const times: Record<string, string> = {};
  for (const [key, at] of Object.entries(value)) { if (key.length > 128 || !timestamp(at)) throw invalid(); times[key] = at; }
  const aliases: Record<string, string> = { battery_percent: 'batteryLevel', estimated_range_km: 'distanceToEmpty', odometer_km: 'vehicleMileage', temperature_celsius: 'cabinClimateInteriorTemperature', charging_state: 'chargerState', limit_percent: 'batteryLimit', location: 'gnssLocation' };
  for (const [key, upstream] of Object.entries(aliases)) if (!times[key] && times[upstream]) times[key] = times[upstream];
  // A newer passenger-door report may affect the composite lock state. Preserve
  // the HTTP result until the stream is newer than every known door report.
  const doors = ['doorFrontLeftLocked', 'doorFrontRightLocked', 'doorRearLeftLocked', 'doorRearRightLocked'].map(key => times[key]).filter(Boolean);
  if (!times.locked && doors.length) times.locked = doors.reduce((latest, at) => Date.parse(at) > Date.parse(latest) ? at : latest);
  return times;
}

/** A different vehicle's result must never be presented under the selected vehicle. */
export function readVehicleState(value: unknown, vehicleId: string, mode: AppMode = 'demo'): Vehicle {
  const vehicle = record(value) ? value.vehicle : undefined;
  if (!record(vehicle) || vehicle.id !== vehicleId || !text(vehicle.name) || !(text(vehicle.model) || allowedMissing(vehicle.model, mode)) ||
    !(numberField(vehicle.model_year, 2000, 2200, mode) && (vehicle.model_year === null || Number.isInteger(vehicle.model_year))) ||
    !numberField(vehicle.battery_percent, 0, 100, mode) || !numberField(vehicle.estimated_range_km, 0, 5000, mode) ||
    !numberField(vehicle.odometer_km, 0, 10_000_000, mode) || !(typeof vehicle.locked === 'boolean' || allowedMissing(vehicle.locked, mode)) ||
    !(text(vehicle.charging_state) || allowedMissing(vehicle.charging_state, mode)) || !numberField(vehicle.temperature_celsius, -100, 100, mode) ||
    !location(vehicle.location, mode) || !(timestamp(vehicle.observed_at) || allowedMissing(vehicle.observed_at, mode)) || vehicle.source !== (mode === 'demo' ? 'synthetic-demo' : 'live')) throw invalid();
  const times = fieldTimes(vehicle.field_observed_at);
  return { ...vehicle, ...(times ? { field_observed_at: times } : {}) } as unknown as Vehicle;
}

export function readChargingStatus(value: unknown, vehicleId: string, mode: AppMode = 'demo'): ChargingStatus {
  if (!record(value) || value.vehicle_id !== vehicleId || !text(value.state) ||
    !numberField(value.battery_percent, 0, 100, mode) || !numberField(value.limit_percent, 0, 100, mode) ||
    !numberField(value.power_kw, 0, 1000, mode)) throw invalid();
  const times = fieldTimes(value.field_observed_at);
  return { ...value, ...(times ? { field_observed_at: times } : {}) } as unknown as ChargingStatus;
}

export function readChargingHistory(value: unknown, vehicleId: string, mode: AppMode = 'demo'): ChargingHistory {
  if (!record(value) || value.vehicle_id !== vehicleId || !Array.isArray(value.sessions) || value.sessions.length > 100 ||
    typeof value.has_more !== 'boolean' || !value.sessions.every(session => record(session) && text(session.id) &&
      (timestamp(session.started_at) || allowedMissing(session.started_at, mode)) && numberField(session.energy_kwh, 0, 1000, mode) && numberField(session.duration_minutes, 0, 100_000, mode)) ||
    new Set(value.sessions.map(session => session.id)).size !== value.sessions.length) throw invalid();
  return value as unknown as ChargingHistory;
}

export function readVehicleLocation(value: unknown, vehicleId: string, mode: AppMode = 'demo'): VehicleLocation {
  if (!record(value) || value.vehicle_id !== vehicleId || !location(value.location, mode)) throw invalid();
  const times = fieldTimes(value.field_observed_at);
  if (value.observed_at !== undefined && value.observed_at !== null && !timestamp(value.observed_at)) throw invalid();
  return { ...value, ...(times || timestamp(value.observed_at) ? { field_observed_at: { ...times, ...(timestamp(value.observed_at) ? { location: value.observed_at } : {}) } } : {}) } as unknown as VehicleLocation;
}

export function sampleDate(value: string | null): string {
  if (!timestamp(value)) return 'Date unavailable';
  return new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric', hour: 'numeric', minute: '2-digit', timeZone: 'UTC' }).format(new Date(value)) + ' UTC';
}

export function ownerNumber(value: number | null, digits = 0): string {
  if (value === null || !Number.isFinite(value)) return 'Unavailable';
  return new Intl.NumberFormat('en-US', { maximumFractionDigits: digits }).format(value);
}

export function chargeLabel(state: string | null): string {
  switch (state) {
    case 'charging': return 'Charging';
    case 'not_charging': return 'Not charging';
    case 'disconnected': return 'Not plugged in';
    case 'complete': return 'Charge complete';
    case 'plugged_in': return 'Plugged in';
    default: return 'Charging state unavailable';
  }
}

export const lockLabel = (locked: boolean | null): string => locked === true ? 'Locked' : locked === false ? 'Unlocked' : 'Lock status unavailable';
