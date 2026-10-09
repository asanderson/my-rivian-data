import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { accountExpired, ApiError } from './api';
import type { AppMode } from './api';
import type { Vehicle, WasmModule } from './domain';
import { Icon } from './Icon';
import { runQuery } from './operations';
import { chargeLabel, lockLabel, ownerNumber, readChargingHistory, readChargingStatus, readVehicleLocation, readVehicleState, sampleDate } from './owner-data';
import type { ChargingHistory, ChargingStatus, VehicleLocation } from './owner-data';
import './owner.css';

export type OwnerArea = 'overview' | 'charging' | 'health' | 'location';
export interface OwnerViewProps {
  mode: AppMode;
  area: OwnerArea;
  vehicles: Vehicle[];
  vehicleId: string;
  onSelectVehicle: (id: string) => void;
  onNavigate: (area: OwnerArea) => void;
  wasm: WasmModule | null;
  wasmState: 'loading' | 'ready' | 'failed';
  onSessionExpired: () => void;
  onAccountExpired: () => void;
}
interface ReadingTime { observedAt: string | null; receivedAt?: string }
type OwnerData = ReadingTime & (
  | { kind: 'vehicle'; vehicle: Vehicle }
  | { kind: 'charging'; status: ChargingStatus; history: ChargingHistory; historyObservedAt: string | null; historyReceivedAt?: string }
  | { kind: 'location'; position: VehicleLocation });
interface LoadState { key: string; data?: OwnerData; failed?: string }

function ReadingTime({ observedAt, receivedAt, mode }: ReadingTime & { mode: AppMode }) {
  return <p className="owner-sample-time"><Icon name="clock" /><span>{mode === 'demo' ? `Sample recorded ${sampleDate(observedAt)}` : <>{receivedAt ? `Retrieved ${sampleDate(receivedAt)}. ` : ''}{observedAt ? `Vehicle reported ${sampleDate(observedAt)}.` : 'Rivian has not supplied one observation time for these readings; they may be older.'}</>}</span></p>;
}
function Metric({ title, value, unit, detail }: { title: string; value: ReactNode; unit?: string; detail?: string }) {
  return <div className="owner-metric"><dt>{title}</dt><dd>{value}{unit && <span>{unit}</span>}</dd>{detail && <p>{detail}</p>}</div>;
}
function NumberMetric({ title, value, unit, digits = 0, detail }: { title: string; value: number | null; unit: string; digits?: number; detail?: string }) {
  return <Metric title={title} value={ownerNumber(value, digits)} unit={value === null ? undefined : unit} detail={detail} />;
}
function Availability({ children }: { children: ReactNode }) {
  return <div className="owner-availability"><Icon name="alert" /><div>{children}</div></div>;
}

function VehicleSummary({ vehicle, mode, observedAt, receivedAt, onNavigate }: ReadingTime & { vehicle: Vehicle; mode: AppMode; onNavigate: (area: OwnerArea) => void }) {
  return <section className="owner-summary" aria-labelledby="owner-selected-heading">
    <div className="owner-summary-top"><div><p className="owner-kicker">{mode === 'demo' ? 'SELECTED SAMPLE VEHICLE' : 'YOUR VEHICLE'}</p><h2 id="owner-selected-heading">{vehicle.name}</h2><p>{[vehicle.model_year, vehicle.model].filter(Boolean).join(' · ') || 'Model details unavailable'}</p></div><span className="owner-car-mark"><Icon name="car" /></span></div>
    <dl className="owner-metrics owner-metrics-three"><NumberMetric title="Battery charge" value={vehicle.battery_percent} unit="%" /><NumberMetric title="Estimated range" value={vehicle.estimated_range_km} unit="km" /><NumberMetric title="Odometer" value={vehicle.odometer_km} unit="km" /></dl>
    {vehicle.battery_percent !== null && <><label className="sr-only" htmlFor="owner-battery">{vehicle.name} {mode === 'demo' ? 'sample ' : ''}battery charge</label><progress id="owner-battery" value={vehicle.battery_percent} max={100} /></>}
    <div className="owner-summary-foot"><div className="owner-state-tags"><span><Icon name="lock" />{mode === 'live' ? 'Passenger doors · ' : ''}{lockLabel(vehicle.locked)}</span><span><Icon name="bolt" />{chargeLabel(vehicle.charging_state)}</span></div><button className="owner-text-link" type="button" onClick={() => onNavigate('charging')}>View charging <Icon name="arrow" /></button></div>
    <ReadingTime mode={mode} observedAt={observedAt} receivedAt={receivedAt} />
  </section>;
}

function ChargingView({ data, mode }: { data: Extract<OwnerData, { kind: 'charging' }>; mode: AppMode }) {
  const energy = data.history.sessions.every(session => session.energy_kwh !== null) ? data.history.sessions.reduce((total, session) => total + session.energy_kwh!, 0) : null;
  return <>
    <section className="owner-panel" aria-labelledby="owner-charging-status"><div className="owner-panel-title"><h2 id="owner-charging-status">Charging status</h2><span className="owner-status"><Icon name="bolt" />{chargeLabel(data.status.state)}</span></div>
      <dl className="owner-metrics owner-metrics-three"><NumberMetric title="Battery charge" value={data.status.battery_percent} unit="%" /><NumberMetric title="Charge limit" value={data.status.limit_percent} unit="%" /><NumberMetric title="Charging power" value={data.status.power_kw} unit="kW" digits={1} /></dl>
      <ReadingTime mode={mode} {...data} /><p className="owner-help">{mode === 'demo' ? 'These are sample readings. Charging settings cannot be changed in this demo.' : 'Charging readings are provided by Rivian. Missing readings are shown as unavailable.'}</p>
    </section>
    <section className="owner-panel" aria-labelledby="owner-charging-history"><div className="owner-panel-title"><div><h2 id="owner-charging-history">Charging history</h2><p>{mode === 'demo' ? 'Fictional charging sessions for this sample vehicle.' : 'Charging sessions Rivian associates with this vehicle, up to 100 at a time.'}</p></div></div>
      <dl className="owner-history-summary"><div><dt>Sessions shown</dt><dd>{data.history.sessions.length}</dd></div><div><dt>Energy added in these sessions</dt><dd>{ownerNumber(energy, 1)} {energy !== null && <span>kWh</span>}</dd></div></dl>
      {data.history.sessions.length === 0 ? <p className="owner-empty">{mode === 'demo' ? 'No sample charging sessions are available.' : 'No charging sessions were returned for this vehicle.'}</p> : <ul className="owner-sessions">{data.history.sessions.map(session => <li key={session.id}><span className="owner-session-icon"><Icon name="bolt" /></span><div className="owner-session-date"><h3>{mode === 'demo' ? 'Sample charging session' : 'Charging session'}</h3><p>{sampleDate(session.started_at)}</p></div><dl><div><dt>Energy added</dt><dd>{ownerNumber(session.energy_kwh, 1)} {session.energy_kwh !== null && <span>kWh</span>}</dd></div><div><dt>Duration</dt><dd>{ownerNumber(session.duration_minutes)} {session.duration_minutes !== null && <span>min</span>}</dd></div></dl></li>)}</ul>}
      {data.history.has_more && <p className="owner-help">More sessions may be available than are shown here.</p>}<ReadingTime mode={mode} observedAt={data.historyObservedAt} receivedAt={data.historyReceivedAt} />
      <Availability><p>{mode === 'demo' ? 'Charging cost, station details, and charging efficiency are not available in this demo.' : 'Rivian may omit sessions or leave readings empty. Sessions without a matching vehicle are not assigned to your vehicle. This list is not a complete billing record.'}</p></Availability>
    </section>
  </>;
}

function HealthView({ vehicle, mode, observedAt, receivedAt }: ReadingTime & { vehicle: Vehicle; mode: AppMode }) {
  return <>
    <section className="owner-panel" aria-labelledby="owner-health-readings"><div className="owner-panel-title"><div><h2 id="owner-health-readings">Available readings</h2><p>{mode === 'demo' ? 'A few sample readings, without a vehicle health assessment.' : 'Reported readings, without a vehicle health assessment.'}</p></div></div>
      <dl className="owner-metrics owner-metrics-three"><NumberMetric title="Odometer" value={vehicle.odometer_km} unit="km" /><NumberMetric title="Reported temperature" value={vehicle.temperature_celsius} digits={1} unit="°C" detail={mode === 'demo' ? 'The sample does not identify a sensor.' : 'Cabin temperature when supplied by Rivian.'} /><Metric title={mode === 'demo' ? 'Lock status' : 'Passenger doors'} value={lockLabel(vehicle.locked)} detail={mode === 'live' ? 'The four passenger doors only; excludes frunk and tailgate.' : undefined} /></dl><ReadingTime mode={mode} observedAt={observedAt} receivedAt={receivedAt} />
    </section>
    <section className="owner-panel" aria-labelledby="owner-health-unavailable"><div className="owner-panel-title"><h2 id="owner-health-unavailable">What is not available yet</h2></div><div className="owner-unavailable-grid">{[
      ['Diagnostics & alerts', 'No fault codes or diagnostic results are included in this view.'],
      ['Service history', 'Maintenance records are not included in this view.'],
      ['Tire pressure', 'Tire readings are not presented in this view.'],
      ['Battery health', 'Battery charge is available; battery condition and degradation are not.'],
    ].map(([title, detail]) => <div className="owner-unavailable-item" key={title}><h3>{title}</h3><span>Not available</span><p>{detail}</p></div>)}</div><Availability><p>This {mode === 'demo' ? 'demo' : 'view'} cannot tell you whether a vehicle needs service. Use your Rivian app and vehicle for actual alerts.</p></Availability></section>
  </>;
}

function LocationView({ data, mode }: { data: Extract<OwnerData, { kind: 'location' }>; mode: AppMode }) {
  const position = data.position.location;
  return <section className="owner-panel owner-location" aria-labelledby="owner-location-details"><div className="owner-location-illustration"><Icon name="pin" /><span>{mode === 'demo' ? 'Fictional location' : 'Vehicle location'}</span><p>{mode === 'demo' ? 'No map or real location is loaded.' : 'Coordinates stay in your local workspace. No external map is loaded.'}</p></div><div className="owner-location-details"><p className="owner-kicker">{mode === 'demo' ? 'SAMPLE ONLY' : 'REPORTED BY RIVIAN'}</p><h2 id="owner-location-details">Location details</h2><p>{mode === 'demo' ? 'These made-up coordinates show how a vehicle location can be presented.' : 'The last position returned by Rivian may differ from the current vehicle position.'}</p>{position ? <dl className="owner-location-values"><div><dt>Latitude</dt><dd>{position.latitude.toFixed(4)}°</dd></div><div><dt>Longitude</dt><dd>{position.longitude.toFixed(4)}°</dd></div><div><dt>Reported accuracy</dt><dd>{position.accuracy_m === null ? 'Unavailable' : `${ownerNumber(position.accuracy_m)} m`}</dd></div></dl> : <Availability><p>Rivian did not return a location for this vehicle.</p></Availability>}<ReadingTime mode={mode} {...data} />{mode === 'demo' && <Availability><p>Real vehicle locations and directions are not available in this demo.</p></Availability>}</div></section>;
}

const areaCopy = {
  overview: ['Your vehicles. Your data.', 'A simple place to understand your Rivian, organized around the things you care about.'],
  charging: ['Charging', 'See battery charge, charging status, and a history of charging sessions.'],
  health: ['Vehicle health', 'Explore the available readings and see which information is still missing.'],
  location: ['Location', 'See the location information associated with your vehicle.'],
};

export function OwnerView({ mode, area, vehicles, vehicleId, onSelectVehicle, onNavigate, wasm, wasmState, onSessionExpired, onAccountExpired }: OwnerViewProps) {
  const [attempt, setAttempt] = useState(0);
  const [loaded, setLoaded] = useState<LoadState | null>(null);
  const expired = useRef(onSessionExpired);
  const accountDisconnected = useRef(onAccountExpired);
  expired.current = onSessionExpired;
  accountDisconnected.current = onAccountExpired;
  const key = `${mode}:${area}:${vehicleId}:${attempt}`;
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
          runQuery<unknown>(wasm!, 'charging-status', variables, mode),
          runQuery<unknown>(wasm!, 'charging-history', { ...variables, limit: 100 }, mode),
        ]);
        return { kind: 'charging', status: readChargingStatus(status.data, vehicleId, mode), history: readChargingHistory(history.data, vehicleId, mode), observedAt: status.observedAt, receivedAt: status.receivedAt, historyObservedAt: history.observedAt, historyReceivedAt: history.receivedAt };
      }
      if (area === 'location') {
        const result = await runQuery<unknown>(wasm!, 'vehicle-location', variables, mode);
        return { kind: 'location', position: readVehicleLocation(result.data, vehicleId, mode), observedAt: result.observedAt, receivedAt: result.receivedAt };
      }
      const result = await runQuery<unknown>(wasm!, 'vehicle-state', variables, mode);
      return { kind: 'vehicle', vehicle: readVehicleState(result.data, vehicleId, mode), observedAt: result.observedAt, receivedAt: result.receivedAt };
    }
    void load().then(data => { if (!cancelled) setLoaded({ key, data }); }).catch((error: unknown) => {
      if (cancelled) return;
      if (accountExpired(error)) { accountDisconnected.current(); return; }
      if (error instanceof ApiError && error.status === 401) { expired.current(); return; }
      setLoaded({ key, failed: error instanceof Error ? error.message : 'Please try again.' });
    });
    return () => { cancelled = true; };
  }, [mode, area, vehicleId, wasm, wasmState, key]);

  return <div className="owner-view">
    <header className="owner-heading"><p className="owner-kicker">{area === 'overview' ? 'MADE FOR OWNERS' : `${vehicle?.name ?? 'Your vehicle'}${mode === 'demo' ? ' · SAMPLE DATA' : ''}`}</p><h1>{areaCopy[area][0]}</h1><p>{areaCopy[area][1]}</p>{vehicle && <button className="button button-secondary owner-refresh" type="button" disabled={!active || wasmState !== 'ready'} onClick={() => setAttempt(value => value + 1)}>Refresh data</button>}</header>
    {area === 'overview' && <section className="owner-vehicle-picker" aria-label={mode === 'demo' ? 'Your sample vehicles' : 'Your vehicles'}>{vehicles.map(item => <button key={item.id} className={`owner-vehicle-choice ${item.id === vehicleId ? 'selected' : ''}`} type="button" aria-pressed={item.id === vehicleId} onClick={() => onSelectVehicle(item.id)}><Icon name="car" /><span><strong>{item.name}</strong><small>{[item.model_year, item.model, mode === 'demo' ? 'Sample vehicle' : null].filter(Boolean).join(' · ')}</small></span>{item.id === vehicleId && <Icon name="check" />}</button>)}</section>}
    {!vehicle ? <div className="owner-load-state" role="status"><Icon name="car" /><h2>No vehicle selected</h2><p>Choose a vehicle to explore its information.</p></div> : wasmState === 'failed' ? <div className="owner-load-state" role="alert"><Icon name="alert" /><h2>Vehicle information is unavailable</h2><p>Close this page and relaunch My Rivian Data to try again.</p></div> : active?.failed ? <div className="owner-load-state" role="alert"><Icon name="alert" /><h2>{mode === 'demo' ? 'We could not load this sample' : 'We could not load this information'}</h2><p>{mode === 'demo' ? 'Check that My Rivian Data is still running, then try again.' : active.failed}</p><button className="button button-secondary" type="button" onClick={() => setAttempt(value => value + 1)}>Try again</button></div> : !active?.data ? <div className="owner-load-state" role="status"><Icon name="clock" /><h2>{mode === 'demo' ? 'Loading sample information…' : 'Loading your vehicle information…'}</h2><p>Getting the available readings for {vehicle.name}.</p></div> : <div className="owner-data" aria-label={`${vehicle.name}${mode === 'demo' ? ' sample' : ''} information`}>
      {area === 'overview' && active.data.kind === 'vehicle' && <VehicleSummary mode={mode} {...active.data} onNavigate={onNavigate} />}
      {area === 'charging' && active.data.kind === 'charging' && <ChargingView mode={mode} data={active.data} />}
      {area === 'health' && active.data.kind === 'vehicle' && <HealthView mode={mode} {...active.data} />}
      {area === 'location' && active.data.kind === 'location' && <LocationView mode={mode} data={active.data} />}
    </div>}
    {area === 'overview' && <section className="owner-explore" aria-labelledby="owner-explore-heading"><div className="owner-panel-title"><h2 id="owner-explore-heading">Explore your vehicle</h2></div><div className="owner-area-grid">{([
      ['charging', 'bolt', 'Charging', 'Battery charge, charging status, and charging history.'],
      ['health', 'heart', 'Vehicle health', 'Available readings and a clear view of what is missing.'],
      ['location', 'pin', 'Location', mode === 'demo' ? 'An example of location information, using fictional coordinates.' : 'The last vehicle position returned by Rivian.'],
    ] as const).map(([target, icon, title, description]) => <button className="owner-area-card" key={target} type="button" onClick={() => onNavigate(target)}><span className="owner-area-icon"><Icon name={icon} /></span><h3>{title}</h3><p>{description}</p><span className="owner-area-action">View {target === 'health' ? 'vehicle health' : title.toLowerCase()}<Icon name="arrow" /></span></button>)}</div></section>}
  </div>;
}
