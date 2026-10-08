import type { Vehicle } from './domain.ts';

export interface ChargingStatus {
  vehicle_id: string;
  state: string;
  battery_percent: number;
  limit_percent: number;
  power_kw: number;
}
export interface ChargingSession {
  id: string;
  started_at: string;
  energy_kwh: number;
  duration_minutes: number;
}
export interface ChargingHistory {
  vehicle_id: string;
  sessions: ChargingSession[];
  has_more: boolean;
}
export interface VehicleLocation { vehicle_id: string; location: Vehicle['location'] }

const invalid = () => new Error('This sample could not be displayed. Please try again.');
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const bounded = (value: unknown, min: number, max: number): value is number => typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
const text = (value: unknown): value is string => typeof value === 'string' && value.length > 0 && value.length <= 256;
const timestamp = (value: unknown): value is string => text(value) && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value) && Number.isFinite(Date.parse(value));
const location = (value: unknown): value is Vehicle['location'] => record(value) && bounded(value.latitude, -90, 90) && bounded(value.longitude, -180, 180) && bounded(value.accuracy_m, 0, 100_000);

/** A different vehicle's result must never be presented under the selected vehicle. */
export function readVehicleState(value: unknown, vehicleId: string): Vehicle {
  const vehicle = record(value) ? value.vehicle : undefined;
  if (!record(vehicle) || vehicle.id !== vehicleId || !text(vehicle.name) || !text(vehicle.model) ||
    !bounded(vehicle.model_year, 2000, 2200) || !Number.isInteger(vehicle.model_year) ||
    !bounded(vehicle.battery_percent, 0, 100) || !bounded(vehicle.estimated_range_km, 0, 5000) ||
    !bounded(vehicle.odometer_km, 0, 10_000_000) || typeof vehicle.locked !== 'boolean' ||
    !text(vehicle.charging_state) || !bounded(vehicle.temperature_celsius, -100, 100) ||
    !location(vehicle.location) || !timestamp(vehicle.observed_at) || vehicle.source !== 'synthetic-demo') throw invalid();
  return vehicle as unknown as Vehicle;
}

export function readChargingStatus(value: unknown, vehicleId: string): ChargingStatus {
  if (!record(value) || value.vehicle_id !== vehicleId || !text(value.state) ||
    !bounded(value.battery_percent, 0, 100) || !bounded(value.limit_percent, 0, 100) ||
    !bounded(value.power_kw, 0, 1000)) throw invalid();
  return value as unknown as ChargingStatus;
}

export function readChargingHistory(value: unknown, vehicleId: string): ChargingHistory {
  if (!record(value) || value.vehicle_id !== vehicleId || !Array.isArray(value.sessions) || value.sessions.length > 100 ||
    typeof value.has_more !== 'boolean' || !value.sessions.every(session => record(session) && text(session.id) &&
      timestamp(session.started_at) && bounded(session.energy_kwh, 0, 1000) && bounded(session.duration_minutes, 0, 100_000)) ||
    new Set(value.sessions.map(session => session.id)).size !== value.sessions.length) throw invalid();
  return value as unknown as ChargingHistory;
}

export function readVehicleLocation(value: unknown, vehicleId: string): VehicleLocation {
  if (!record(value) || value.vehicle_id !== vehicleId || !location(value.location)) throw invalid();
  return value as unknown as VehicleLocation;
}

export function sampleDate(value: string): string {
  if (!timestamp(value)) return 'Sample date unavailable';
  return new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric', hour: 'numeric', minute: '2-digit', timeZone: 'UTC' }).format(new Date(value)) + ' UTC';
}

export function ownerNumber(value: number, digits = 0): string {
  return new Intl.NumberFormat('en-US', { maximumFractionDigits: digits }).format(value);
}

export function chargeLabel(state: string): string {
  switch (state) {
    case 'charging': return 'Charging';
    case 'not_charging': return 'Not charging';
    case 'disconnected': return 'Not plugged in';
    case 'complete': return 'Charge complete';
    case 'plugged_in': return 'Plugged in';
    default: return 'Charging state unavailable';
  }
}
