import { useEffect, useRef, useState } from 'react';
import { ApiError, request } from './api';
import { createTelemetryPoller, readTelemetrySnapshot, telemetryAuthenticationExpired } from './telemetry';
import type { TelemetrySnapshot } from './telemetry';

// Serial lifecycle operations prevent an old view's stop from racing a new start.
let lifecycle: Promise<unknown> = Promise.resolve();
function serialize(action: () => Promise<unknown>): Promise<unknown> {
  const result = lifecycle.catch(() => {}).then(action);
  lifecycle = result.catch(() => {});
  return result;
}
export interface LiveConnection {
  enabled: boolean; snapshot: TelemetrySnapshot | null; problem: string; busy: boolean;
  setEnabled: (enabled: boolean) => void;
}
export function useTelemetry(vehicleId: string, accountKey: number, eligible: boolean, onAuthError: (error: unknown) => void): LiveConnection {
  const [enabled, setEnabled] = useState(false);
  const [loaded, setLoaded] = useState<{ key: string; snapshot: TelemetrySnapshot | null; problem: string; busy: boolean } | null>(null);
  const failure = useRef(onAuthError); failure.current = onAuthError;
  const key = `${accountKey}:${vehicleId}:${eligible}:${enabled}`;
  const active = loaded?.key === key ? loaded : null;
  useEffect(() => { setEnabled(false); }, [accountKey]);
  useEffect(() => {
    if (!eligible || !enabled || !vehicleId) return;
    let closed = false;
    const controller = new AbortController();
    const poller = createTelemetryPoller(vehicleId, snapshot => {
      if (snapshot.state === 'authentication_required') {
        setEnabled(false);
        if (telemetryAuthenticationExpired(snapshot)) failure.current(new ApiError(401, 'account_auth_required', 'Sign in to Rivian again for live updates.'));
      }
      setLoaded({ key, snapshot, problem: '', busy: false });
    }, error => { setLoaded({ key, snapshot: null, problem: error instanceof Error ? error.message : 'Live updates are unavailable.', busy: false }); failure.current(error); }, {
      get: () => request('/api/telemetry', undefined, controller.signal), schedule: (callback, delay) => window.setTimeout(callback, delay), cancel: timer => window.clearTimeout(timer),
    });
    setLoaded({ key, snapshot: null, problem: '', busy: true });
    void serialize(async () => {
      if (closed) return;
      const value = await request('/api/telemetry/start', { vehicle_id: vehicleId });
      if (closed) return;
      const snapshot = readTelemetrySnapshot(value, vehicleId);
      setLoaded({ key, snapshot, problem: '', busy: false });
      void poller.poll();
    }).catch(error => {
      if (!closed) { setLoaded({ key, snapshot: null, problem: error instanceof Error ? error.message : 'Live updates could not start.', busy: false }); failure.current(error); }
    });
    return () => { closed = true; poller.close(); controller.abort(); void serialize(() => request('/api/telemetry/stop', {})).catch(() => {}); };
  }, [key, eligible, enabled, vehicleId]);
  return { enabled, setEnabled, snapshot: active?.snapshot ?? null, problem: active?.problem ?? '', busy: active?.busy ?? false };
}
