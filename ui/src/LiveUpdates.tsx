import type { LiveConnection } from './useTelemetry';
import { sampleDate } from './owner-data';

const states = { stopped: 'Stopped', connecting: 'Connecting', connected: 'Connected', reconnecting: 'Reconnecting', authentication_required: 'Sign-in required', error: 'Unavailable' };
const names: Record<string, string> = { battery_percent: 'Battery charge', estimated_range_km: 'Estimated range', odometer_km: 'Odometer', locked: 'Passenger door locks', charging_state: 'Charging status', temperature_celsius: 'Cabin temperature', location: 'Location', limit_percent: 'Charge limit', power_kw: 'Charging power' };
export function LiveUpdates({ live, developer = false }: { live: LiveConnection; developer?: boolean }) {
  const readings = live.snapshot?.readings;
  const times = readings ? Object.entries(readings.field_observed_at).filter(([key]) => key in names && readings[key as keyof typeof readings] !== null) : [];
  return <section className="live-updates" aria-label="Live updates">
    <div className="live-updates-heading"><div><h2>Live updates</h2><p role="status">{live.busy ? 'Connecting…' : live.enabled ? states[live.snapshot?.state ?? 'connecting'] : 'Stopped'}</p></div><button className="button button-secondary" type="button" disabled={live.busy} onClick={() => live.setEnabled(!live.enabled)}>{live.enabled ? 'Stop live updates' : 'Start live updates'}</button></div>
    <p>Updates follow your selected vehicle. Other readings keep their last retrieved values until the vehicle reports them. Connection compatibility has not been verified with a real vehicle.</p>
    {live.problem && <p className="error-message" role="alert">{live.problem}</p>}
    {live.snapshot?.error_code && <p role="status">The live connection is unavailable. Stop and start updates to retry.</p>}
    {times.length > 0 && <details><summary>When the vehicle reported live readings</summary><dl className="live-reading-times">{times.map(([key, at]) => <div key={key}><dt>{names[key]}</dt><dd>{sampleDate(at)}</dd></div>)}</dl></details>}
    {developer && <details className="developer-request-preview"><summary>Native stream snapshot and raw topics</summary><p>Bounded native snapshot. Unexpected topics are discarded before they reach this view.</p><pre className="developer-stream-json" tabIndex={0}>{JSON.stringify(live.snapshot, null, 2)}</pre></details>}
  </section>;
}
