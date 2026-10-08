import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { ApiError } from './api';
import type { Vehicle, WasmModule } from './domain';
import { Icon } from './Icon';
import { runDemoQuery } from './operations';
import { chargeLabel, ownerNumber, readChargingHistory, readChargingStatus, readVehicleLocation, readVehicleState, sampleDate } from './owner-data';
import type { ChargingHistory, ChargingStatus, VehicleLocation } from './owner-data';
import './owner.css';

export type OwnerArea = 'overview' | 'charging' | 'health' | 'location';
export interface OwnerViewProps {
  area: OwnerArea;
  vehicles: Vehicle[];
  vehicleId: string;
  onSelectVehicle: (id: string) => void;
  onNavigate: (area: OwnerArea) => void;
  wasm: WasmModule | null;
  wasmState: 'loading' | 'ready' | 'failed';
  onSessionExpired: () => void;
}
type OwnerData =
  | { kind: 'vehicle'; vehicle: Vehicle; observedAt: string }
  | { kind: 'charging'; status: ChargingStatus; history: ChargingHistory; observedAt: string; historyObservedAt: string }
  | { kind: 'location'; position: VehicleLocation; observedAt: string };
interface LoadState { key: string; data?: OwnerData; failed?: boolean }

function SampleTime({ value }: { value: string }) {
  return <p className="owner-sample-time"><Icon name="clock" /><span>Sample recorded {sampleDate(value)}</span></p>;
}
function Metric({ title, value, unit, detail }: { title: string; value: ReactNode; unit?: string; detail?: string }) {
  return <div className="owner-metric"><dt>{title}</dt><dd>{value}{unit && <span>{unit}</span>}</dd>{detail && <p>{detail}</p>}</div>;
}
function Availability({ children }: { children: ReactNode }) {
  return <div className="owner-availability"><Icon name="alert" /><div>{children}</div></div>;
}

function VehicleSummary({ vehicle, observedAt, onNavigate }: { vehicle: Vehicle; observedAt: string; onNavigate: (area: OwnerArea) => void }) {
  return <section className="owner-summary" aria-labelledby="owner-selected-heading">
    <div className="owner-summary-top"><div><p className="owner-kicker">SELECTED SAMPLE VEHICLE</p><h2 id="owner-selected-heading">{vehicle.name}</h2><p>{vehicle.model_year} · {vehicle.model}</p></div><span className="owner-car-mark"><Icon name="car" /></span></div>
    <dl className="owner-metrics owner-metrics-three"><Metric title="Battery charge" value={ownerNumber(vehicle.battery_percent)} unit="%" /><Metric title="Estimated range" value={ownerNumber(vehicle.estimated_range_km)} unit="km" /><Metric title="Odometer" value={ownerNumber(vehicle.odometer_km)} unit="km" /></dl>
    <label className="sr-only" htmlFor="owner-battery">{vehicle.name} sample battery charge</label><progress id="owner-battery" value={vehicle.battery_percent} max={100} />
    <div className="owner-summary-foot"><div className="owner-state-tags"><span><Icon name="lock" />{vehicle.locked ? 'Locked' : 'Unlocked'}</span><span><Icon name="bolt" />{chargeLabel(vehicle.charging_state)}</span></div><button className="owner-text-link" type="button" onClick={() => onNavigate('charging')}>View charging <Icon name="arrow" /></button></div>
    <SampleTime value={observedAt} />
  </section>;
}

function ChargingView({ data }: { data: Extract<OwnerData, { kind: 'charging' }> }) {
  const energy = data.history.sessions.reduce((total, session) => total + session.energy_kwh, 0);
  return <>
    <section className="owner-panel" aria-labelledby="owner-charging-status"><div className="owner-panel-title"><h2 id="owner-charging-status">Charging status</h2><span className="owner-status"><Icon name="bolt" />{chargeLabel(data.status.state)}</span></div>
      <dl className="owner-metrics owner-metrics-three"><Metric title="Battery charge" value={ownerNumber(data.status.battery_percent)} unit="%" /><Metric title="Charge limit" value={ownerNumber(data.status.limit_percent)} unit="%" /><Metric title="Charging power" value={ownerNumber(data.status.power_kw, 1)} unit="kW" /></dl>
      <SampleTime value={data.observedAt} /><p className="owner-help">These are sample readings. Charging settings cannot be changed in this demo.</p>
    </section>
    <section className="owner-panel" aria-labelledby="owner-charging-history"><div className="owner-panel-title"><div><h2 id="owner-charging-history">Charging history</h2><p>Fictional charging sessions for this sample vehicle.</p></div></div>
      <dl className="owner-history-summary"><div><dt>Sessions shown</dt><dd>{data.history.sessions.length}</dd></div><div><dt>Energy added in these sessions</dt><dd>{ownerNumber(energy, 1)} <span>kWh</span></dd></div></dl>
      {data.history.sessions.length === 0 ? <p className="owner-empty">No sample charging sessions are available.</p> : <ul className="owner-sessions">{data.history.sessions.map(session => <li key={session.id}><span className="owner-session-icon"><Icon name="bolt" /></span><div className="owner-session-date"><h3>Sample charging session</h3><p>{sampleDate(session.started_at)}</p></div><dl><div><dt>Energy added</dt><dd>{ownerNumber(session.energy_kwh, 1)} <span>kWh</span></dd></div><div><dt>Duration</dt><dd>{ownerNumber(session.duration_minutes)} <span>min</span></dd></div></dl></li>)}</ul>}
      {data.history.has_more && <p className="owner-help">More sample sessions exist than are shown here.</p>}<SampleTime value={data.historyObservedAt} />
      <Availability><p>Charging cost, station details, and charging efficiency are not available in this demo.</p></Availability>
    </section>
  </>;
}

function HealthView({ vehicle, observedAt }: { vehicle: Vehicle; observedAt: string }) {
  return <>
    <section className="owner-panel" aria-labelledby="owner-health-readings"><div className="owner-panel-title"><div><h2 id="owner-health-readings">Available readings</h2><p>A few sample readings, without a vehicle health assessment.</p></div></div>
      <dl className="owner-metrics owner-metrics-three"><Metric title="Odometer" value={ownerNumber(vehicle.odometer_km)} unit="km" /><Metric title="Reported temperature" value={ownerNumber(vehicle.temperature_celsius, 1)} unit="°C" detail="The sample does not identify a sensor." /><Metric title="Lock status" value={vehicle.locked ? 'Locked' : 'Unlocked'} /></dl><SampleTime value={observedAt} />
    </section>
    <section className="owner-panel" aria-labelledby="owner-health-unavailable"><div className="owner-panel-title"><h2 id="owner-health-unavailable">What is not available yet</h2></div><div className="owner-unavailable-grid">{[
      ['Diagnostics & alerts', 'No fault codes or diagnostic results are included.'],
      ['Service history', 'Maintenance records are not included.'],
      ['Tire pressure', 'No tire readings are included.'],
      ['Battery health', 'Battery charge is available; battery condition and degradation are not.'],
    ].map(([title, detail]) => <div className="owner-unavailable-item" key={title}><h3>{title}</h3><span>Not available</span><p>{detail}</p></div>)}</div><Availability><p>This demo cannot tell you whether a vehicle needs service. Use your Rivian app and vehicle for actual alerts.</p></Availability></section>
  </>;
}

function LocationView({ data }: { data: Extract<OwnerData, { kind: 'location' }> }) {
  return <section className="owner-panel owner-location" aria-labelledby="owner-location-details"><div className="owner-location-illustration"><Icon name="pin" /><span>Fictional location</span><p>No map or real location is loaded.</p></div><div className="owner-location-details"><p className="owner-kicker">SAMPLE ONLY</p><h2 id="owner-location-details">Location details</h2><p>These made-up coordinates show how a vehicle location can be presented.</p><dl className="owner-location-values"><div><dt>Latitude</dt><dd>{data.position.location.latitude.toFixed(4)}°</dd></div><div><dt>Longitude</dt><dd>{data.position.location.longitude.toFixed(4)}°</dd></div><div><dt>Reported accuracy</dt><dd>{ownerNumber(data.position.location.accuracy_m)} m</dd></div></dl><SampleTime value={data.observedAt} /><Availability><p>Real vehicle locations and directions are not available in this demo.</p></Availability></div></section>;
}

const areaCopy = {
  overview: ['Your vehicles. Your data.', 'A simple place to understand your Rivian, organized around the things you care about.'],
  charging: ['Charging', 'See battery charge, charging status, and a history of charging sessions.'],
  health: ['Vehicle health', 'Explore the available readings and see which information is still missing.'],
  location: ['Location', 'See where location information would appear, using a fictional sample.'],
};

export function OwnerView({ area, vehicles, vehicleId, onSelectVehicle, onNavigate, wasm, wasmState, onSessionExpired }: OwnerViewProps) {
  const [attempt, setAttempt] = useState(0);
  const [loaded, setLoaded] = useState<LoadState | null>(null);
  const expired = useRef(onSessionExpired);
  expired.current = onSessionExpired;
  const key = `${area}:${vehicleId}:${attempt}`;
  const vehicle = vehicles.find(item => item.id === vehicleId);
  // Render-time association prevents even one frame of an earlier vehicle's data.
  const active = loaded?.key === key && wasmState === 'ready' ? loaded : null;
  useEffect(() => {
    let cancelled = false;
    if (!wasm || wasmState !== 'ready' || !vehicleId) return;
    const variables = { vehicle_id: vehicleId };
    async function load(): Promise<OwnerData> {
      if (area === 'charging') {
        const [status, history] = await Promise.all([
          runDemoQuery<unknown>(wasm!, 'charging-status', variables),
          runDemoQuery<unknown>(wasm!, 'charging-history', { ...variables, limit: 100 }),
        ]);
        return { kind: 'charging', status: readChargingStatus(status.data, vehicleId), history: readChargingHistory(history.data, vehicleId), observedAt: status.observedAt, historyObservedAt: history.observedAt };
      }
      if (area === 'location') {
        const result = await runDemoQuery<unknown>(wasm!, 'vehicle-location', variables);
        return { kind: 'location', position: readVehicleLocation(result.data, vehicleId), observedAt: result.observedAt };
      }
      const result = await runDemoQuery<unknown>(wasm!, 'vehicle-state', variables);
      return { kind: 'vehicle', vehicle: readVehicleState(result.data, vehicleId), observedAt: result.observedAt };
    }
    void load().then(data => { if (!cancelled) setLoaded({ key, data }); }).catch((error: unknown) => {
      if (cancelled) return;
      if (error instanceof ApiError && error.status === 401) { expired.current(); return; }
      setLoaded({ key, failed: true });
    });
    return () => { cancelled = true; };
  }, [area, vehicleId, wasm, wasmState, key]);

  return <div className="owner-view">
    <header className="owner-heading"><p className="owner-kicker">{area === 'overview' ? 'MADE FOR OWNERS' : `${vehicle?.name ?? 'Your vehicle'} · SAMPLE DATA`}</p><h1>{areaCopy[area][0]}</h1><p>{areaCopy[area][1]}</p></header>
    {area === 'overview' && <section className="owner-vehicle-picker" aria-label="Your sample vehicles">{vehicles.map(item => <button key={item.id} className={`owner-vehicle-choice ${item.id === vehicleId ? 'selected' : ''}`} type="button" aria-pressed={item.id === vehicleId} onClick={() => onSelectVehicle(item.id)}><Icon name="car" /><span><strong>{item.name}</strong><small>{item.model_year} · {item.model} · Sample vehicle</small></span>{item.id === vehicleId && <Icon name="check" />}</button>)}</section>}
    {!vehicle ? <div className="owner-load-state" role="status"><Icon name="car" /><h2>No vehicle selected</h2><p>Choose a sample vehicle to explore its information.</p></div> : wasmState === 'failed' ? <div className="owner-load-state" role="alert"><Icon name="alert" /><h2>Sample information is unavailable</h2><p>Close this page and relaunch My Rivian Data to try again.</p></div> : active?.failed ? <div className="owner-load-state" role="alert"><Icon name="alert" /><h2>We could not load this sample</h2><p>Check that My Rivian Data is still running, then try again.</p><button className="button button-secondary" type="button" onClick={() => setAttempt(value => value + 1)}>Try again</button></div> : !active?.data ? <div className="owner-load-state" role="status"><Icon name="clock" /><h2>Loading sample information…</h2><p>Getting the sample for {vehicle.name}.</p></div> : <div className="owner-data" aria-label={`${vehicle.name} sample information`}>
      {area === 'overview' && active.data.kind === 'vehicle' && <VehicleSummary vehicle={active.data.vehicle} observedAt={active.data.observedAt} onNavigate={onNavigate} />}
      {area === 'charging' && active.data.kind === 'charging' && <ChargingView data={active.data} />}
      {area === 'health' && active.data.kind === 'vehicle' && <HealthView vehicle={active.data.vehicle} observedAt={active.data.observedAt} />}
      {area === 'location' && active.data.kind === 'location' && <LocationView data={active.data} />}
    </div>}
    {area === 'overview' && <section className="owner-explore" aria-labelledby="owner-explore-heading"><div className="owner-panel-title"><h2 id="owner-explore-heading">Explore your vehicle</h2></div><div className="owner-area-grid">{([
      ['charging', 'bolt', 'Charging', 'Battery charge, charging status, and sample charging history.'],
      ['health', 'heart', 'Vehicle health', 'Available readings and a clear view of what is missing.'],
      ['location', 'pin', 'Location', 'An example of location information, using fictional coordinates.'],
    ] as const).map(([target, icon, title, description]) => <button className="owner-area-card" key={target} type="button" onClick={() => onNavigate(target)}><span className="owner-area-icon"><Icon name={icon} /></span><h3>{title}</h3><p>{description}</p><span className="owner-area-action">View {target === 'health' ? 'vehicle health' : title.toLowerCase()}<Icon name="arrow" /></span></button>)}</div></section>}
  </div>;
}
