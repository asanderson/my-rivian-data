import { Component, useEffect, useState } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { ApiError, clearSession, request, startSession } from './api';
import type { Operation, Vehicle, WasmModule } from './domain';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { OwnerView } from './OwnerView';
import type { OwnerArea } from './OwnerView';
import { DeveloperView } from './DeveloperView';
import './styles.css';

// Consume the launcher capability before rendering. Never persist it or put it in navigation.
let launcherToken: string | null = null;
if (window.location.hash) {
  launcherToken = new URLSearchParams(window.location.hash.slice(1)).get('bootstrap');
  window.history.replaceState(null, '', window.location.pathname + window.location.search);
}

type View = OwnerArea | 'developer';
type Session = 'loading' | 'ready' | 'ending' | 'ended' | 'error';
const ownerNavigation: { id: OwnerArea; label: string; icon: IconName }[] = [
  { id: 'overview', label: 'Overview', icon: 'grid' },
  { id: 'charging', label: 'Charging', icon: 'bolt' },
  { id: 'health', label: 'Vehicle health', icon: 'heart' },
  { id: 'location', label: 'Location', icon: 'pin' },
];

class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(_error: Error, _info: ErrorInfo) { /* Do not log data or session material. */ }
  render() {
    if (this.state.failed) return <main className="fatal"><Icon name="alert" /><h1>The app could not continue.</h1><p>Close this page and relaunch My Rivian Data to start a fresh session.</p></main>;
    return this.props.children;
  }
}

function App() {
  const [session, setSession] = useState<Session>('loading');
  const [sessionError, setSessionError] = useState('');
  const [view, setView] = useState<View>('overview');
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [vehicleId, setVehicleId] = useState('');
  const [operations, setOperations] = useState<Operation[]>([]);
  const [wasm, setWasm] = useState<WasmModule | null>(null);
  const [wasmState, setWasmState] = useState<'loading' | 'ready' | 'failed'>('loading');

  useEffect(() => {
    let active = true;
    const token = launcherToken;
    launcherToken = null;
    void (async () => {
      try {
        await startSession(token);
        const [catalog, garage] = await Promise.all([
          request<Operation[]>('/api/catalog'), request<Vehicle[]>('/api/vehicles'),
        ]);
        if (!Array.isArray(catalog) || !Array.isArray(garage) || catalog.length > 256 || garage.length > 64) {
          throw new Error('The app could not load the sample vehicles. Please relaunch it.');
        }
        if (!active) return;
        setOperations(catalog); setVehicles(garage); setVehicleId(garage[0]?.id ?? ''); setSession('ready');
        try {
          const wasmUrl = '/wasm/rivian_wasm.js';
          const module = await import(/* @vite-ignore */ wasmUrl) as WasmModule;
          await module.default();
          if (typeof module.validate_request_json !== 'function') throw new Error('The data check is unavailable.');
          if (active) { setWasm(module); setWasmState('ready'); }
        } catch { if (active) setWasmState('failed'); }
      } catch (error) {
        clearSession();
        if (active) {
          setSession('error');
          setSessionError(error instanceof ApiError && error.status === 401
            ? 'Your session has expired. Relaunch My Rivian Data and use the page it opens.'
            : 'We could not open your sample garage. Keep the app running and try relaunching it.');
        }
      }
    })();
    return () => { active = false; };
  }, []);

  function expireSession() {
    clearSession(); setVehicles([]); setOperations([]); setVehicleId(''); setSession('error');
    setSessionError('Your session has expired. Relaunch My Rivian Data to continue.');
  }

  function navigate(next: View) {
    setView(next);
    window.requestAnimationFrame(() => document.getElementById('main')?.focus({ preventScroll: true }));
    window.scrollTo({ top: 0, behavior: 'instant' });
  }

  async function logout() {
    // Unmount data views while logout is pending so late responses cannot reappear.
    setSession('ending'); setSessionError('');
    try {
      await request<void>('/api/logout', {});
      clearSession(); setVehicles([]); setOperations([]); setVehicleId(''); setSession('ended');
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) expireSession();
      else { setSession('ready'); setSessionError('We could not end the session. Try again, or stop the app in its terminal.'); }
    }
  }

  const viewLabel = view === 'developer' ? 'API explorer' : ownerNavigation.find(item => item.id === view)?.label;
  const ready = session === 'ready';
  return <div className="app-shell">
    <a className="skip-link" href="#main">Skip to main content</a>
    <aside className="sidebar" aria-label="Application navigation">
      <button className="brand" onClick={() => navigate('overview')} aria-label="My Rivian Data home"><span className="brand-mark"><Icon name="leaf" /></span><span>My Rivian <strong>Data</strong><small>YOUR VEHICLES. YOUR DATA.</small></span></button>
      <p className="nav-caption">YOUR VEHICLES</p>
      <nav className="owner-navigation" aria-label="Owner features">{ownerNavigation.map(item => <button key={item.id} className={`nav-link ${view === item.id ? 'active' : ''}`} aria-current={view === item.id ? 'page' : undefined} onClick={() => navigate(item.id)}><Icon name={item.icon} /><span>{item.label}</span></button>)}</nav>
      <div className="sidebar-note"><Icon name="shield" /><h2>Your data, on your device.</h2><p>Explore sample vehicles while we build owner access to Rivian data.</p><span className="status-label"><span className="status-dot" />Offline demo</span></div>
      <div className="developer-navigation"><p className="nav-caption">DEVELOPER TOOLS</p><nav aria-label="Developer tools"><button className={`nav-link ${view === 'developer' ? 'active' : ''}`} aria-current={view === 'developer' ? 'page' : undefined} onClick={() => navigate('developer')}><Icon name="code" /><span>API explorer</span></button></nav></div>
      <p className="sidebar-footer">Independent community project<br />Not affiliated with Rivian</p>
    </aside>
    <div className="main-shell">
      <header className="topbar"><div className="breadcrumb"><span>{view === 'developer' ? 'Developer tools' : 'Your workspace'}</span><span aria-hidden="true">/</span><strong>{viewLabel}</strong></div><div className="topbar-actions">{ready && vehicles.length > 0 && <div className="vehicle-picker"><label htmlFor="vehicle-select">Vehicle</label><select id="vehicle-select" value={vehicleId} onChange={event => setVehicleId(event.target.value)}>{vehicles.map(vehicle => <option key={vehicle.id} value={vehicle.id}>{vehicle.name}</option>)}</select></div>}{ready && <button className="end-session" onClick={() => void logout()}><Icon name="out" /><span>End session</span></button>}</div></header>
      <main id="main" tabIndex={-1}>
        {!ready ? <section className="session-panel" aria-live="polite"><Icon name={session === 'loading' || session === 'ending' ? 'leaf' : 'lock'} /><p className="eyebrow">MY RIVIAN DATA</p><h1>{session === 'loading' ? 'Opening your workspace…' : session === 'ending' ? 'Ending your session…' : session === 'ended' ? 'Your session has ended.' : 'Open a fresh session.'}</h1><p>{session === 'loading' ? 'Connecting to the app on this device.' : session === 'ending' ? 'Closing the sample garage.' : session === 'ended' ? 'Relaunch My Rivian Data to explore the sample vehicles again.' : sessionError}</p><p className="muted">No Rivian account is connected. This prototype uses made-up vehicles and data.</p></section> : <>
          <div className="demo-notice"><Icon name="shield" /><p><strong>Offline demo</strong> Sample vehicles and data only. No account is connected, and nothing here can change your vehicle.</p></div>
          {sessionError && <p className="error-message" role="alert">{sessionError}</p>}
          {view === 'developer'
            ? <DeveloperView operations={operations} vehicleId={vehicleId} wasm={wasm} wasmState={wasmState} onSessionExpired={expireSession} />
            : <OwnerView area={view} vehicles={vehicles} vehicleId={vehicleId} onSelectVehicle={setVehicleId} onNavigate={navigate} wasm={wasm} wasmState={wasmState} onSessionExpired={expireSession} />}
          <footer className="page-footer"><span><Icon name="lock" />On your device · Sample data only</span><span>My Rivian Data is an independent, unofficial project.</span></footer>
        </>}
      </main>
    </div>
  </div>;
}

createRoot(document.getElementById('root')!).render(<ErrorBoundary><App /></ErrorBoundary>);
