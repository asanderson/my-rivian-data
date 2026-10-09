import { Component, useEffect, useRef, useState } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { accountExpired, ApiError, clearSession, readAccountStatus, request, startSession } from './api';
import type { AppMode, AccountStatus } from './api';
import { AccountView } from './AccountView';
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
  const sessionOpen = useRef(true);
  const accountEpoch = useRef(0);
  const [session, setSession] = useState<Session>('loading');
  const [mode, setMode] = useState<AppMode>('live');
  const [account, setAccount] = useState<AccountStatus>({ state: 'signed_out' });
  const [garageLoading, setGarageLoading] = useState(false);
  const [accountRevision, setAccountRevision] = useState(0);
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
        const localSession = await startSession(token);
        const catalog = await request<Operation[]>('/api/catalog');
        let accountState = localSession.mode === 'demo' ? { state: 'authenticated' as const }
          : readAccountStatus(await request('/api/account'));
        let garage: Vehicle[] = [];
        let garageProblem = '';
        if (accountState.state === 'authenticated') {
          try { garage = await request<Vehicle[]>('/api/vehicles'); }
          catch (error) {
            if (accountExpired(error)) { accountState = { state: 'signed_out' }; garageProblem = 'Your Rivian sign-in expired. Sign in again to reconnect.'; }
            else if (error instanceof ApiError && error.status === 401) throw error;
            else garageProblem = error instanceof Error ? error.message : 'Your vehicles could not be loaded. Please retry.';
          }
        }
        if (!Array.isArray(catalog) || !Array.isArray(garage) || catalog.length > 256 || garage.length > 64) {
          throw new Error('The app could not load your vehicles. Please relaunch it.');
        }
        if (!active) return;
        setMode(localSession.mode); setAccount(accountState);
        setOperations(catalog); setVehicles(garage); setVehicleId(garage[0]?.id ?? ''); setSessionError(garageProblem); setSession('ready');
        try {
          const wasmUrl = '/wasm/rivian_wasm.js';
          const module = await import(/* @vite-ignore */ wasmUrl) as WasmModule;
          await module.default();
          if (typeof module.validate_request_json !== 'function' || (localSession.mode === 'live' && typeof module.validate_live_request_json !== 'function')) throw new Error('The data check is unavailable.');
          if (active) { setWasm(module); setWasmState('ready'); }
        } catch { if (active) setWasmState('failed'); }
      } catch (error) {
        clearSession();
        if (active) {
          setSession('error');
          setSessionError(error instanceof ApiError && error.status === 401
            ? 'Your session has expired. Relaunch My Rivian Data and use the page it opens.'
            : 'We could not open your workspace. Keep the app running and try relaunching it.');
        }
      }
    })();
    return () => { active = false; };
  }, []);

  function expireSession() {
    sessionOpen.current = false; accountEpoch.current += 1;
    clearSession(); setVehicles([]); setOperations([]); setVehicleId(''); setSession('error');
    setSessionError('Your session has expired. Relaunch My Rivian Data to continue.');
  }

  async function accountChanged(status: AccountStatus) {
    if (!sessionOpen.current) return;
    const epoch = ++accountEpoch.current;
    setVehicles([]); setVehicleId(''); setAccount(status); setSessionError(''); setAccountRevision(value => value + 1);
    if (status.state !== 'authenticated') return;
    setGarageLoading(true);
    try {
      const garage = await request<Vehicle[]>('/api/vehicles');
      if (!sessionOpen.current || epoch !== accountEpoch.current) return;
      if (!Array.isArray(garage) || garage.length > 64) throw new Error('The vehicle list could not be read.');
      setVehicles(garage); setVehicleId(garage[0]?.id ?? '');
    } catch (error) {
      if (!sessionOpen.current || epoch !== accountEpoch.current) return;
      if (accountExpired(error)) accountRequired();
      else if (error instanceof ApiError && error.status === 401) expireSession();
      else setSessionError(error instanceof Error ? error.message : 'Your vehicles could not be loaded. Please retry.');
    } finally { if (sessionOpen.current && epoch === accountEpoch.current) setGarageLoading(false); }
  }

  function accountRequired() {
    accountEpoch.current += 1; setGarageLoading(false);
    setVehicles([]); setVehicleId(''); setAccount({ state: 'signed_out' }); setAccountRevision(value => value + 1);
    setSessionError('Your Rivian sign-in expired. Sign in again to reconnect.');
  }

  async function disconnect() {
    const epoch = ++accountEpoch.current;
    setVehicles([]); setVehicleId(''); setGarageLoading(true); setAccountRevision(value => value + 1);
    try { await request('/api/account/logout', {}); if (!sessionOpen.current || epoch !== accountEpoch.current) return; setAccount({ state: 'signed_out' }); setSessionError(''); }
    catch (error) {
      if (!sessionOpen.current || epoch !== accountEpoch.current) return;
      if (error instanceof ApiError && error.status === 401) expireSession();
      else setSessionError('We could not disconnect. Stop the app to clear your account session, or try again.');
    } finally { if (sessionOpen.current && epoch === accountEpoch.current) setGarageLoading(false); }
  }

  function navigate(next: View) {
    setView(next);
    window.requestAnimationFrame(() => document.getElementById('main')?.focus({ preventScroll: true }));
    window.scrollTo({ top: 0, behavior: 'instant' });
  }

  async function logout() {
    sessionOpen.current = false; accountEpoch.current += 1;
    // Unmount data views while logout is pending so late responses cannot reappear.
    setSession('ending'); setSessionError('');
    try {
      await request<void>('/api/logout', {});
      clearSession(); setVehicles([]); setOperations([]); setVehicleId(''); setSession('ended');
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) expireSession();
      else { sessionOpen.current = true; setGarageLoading(false); setSession('ready'); setSessionError('We could not end the session. Try again, or stop the app in its terminal.'); }
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
      <div className="sidebar-note"><Icon name="shield" /><h2>Your data, on your device.</h2><p>{mode === 'demo' ? 'Explore sample vehicles with no account connection.' : 'Connect directly to Rivian from the app on this device.'}</p><span className="status-label"><span className="status-dot" />{mode === 'demo' ? 'Offline demo' : 'Local workspace'}</span></div>
      <div className="developer-navigation"><p className="nav-caption">DEVELOPER TOOLS</p><nav aria-label="Developer tools"><button className={`nav-link ${view === 'developer' ? 'active' : ''}`} aria-current={view === 'developer' ? 'page' : undefined} onClick={() => navigate('developer')}><Icon name="code" /><span>API explorer</span></button></nav></div>
      <p className="sidebar-footer">Independent community project<br />Not affiliated with Rivian</p>
    </aside>
    <div className="main-shell">
      <header className="topbar"><div className="breadcrumb"><span>{view === 'developer' ? 'Developer tools' : 'Your workspace'}</span><span aria-hidden="true">/</span><strong>{viewLabel}</strong></div><div className="topbar-actions">{ready && vehicles.length > 0 && <div className="vehicle-picker"><label htmlFor="vehicle-select">Vehicle</label><select id="vehicle-select" value={vehicleId} onChange={event => setVehicleId(event.target.value)}>{vehicles.map(vehicle => <option key={vehicle.id} value={vehicle.id}>{vehicle.name}</option>)}</select></div>}{ready && <button className="end-session" onClick={() => void logout()}><Icon name="out" /><span>End session</span></button>}</div></header>
      <main id="main" tabIndex={-1}>
        {!ready ? <section className="session-panel" aria-live="polite"><Icon name={session === 'loading' || session === 'ending' ? 'leaf' : 'lock'} /><p className="eyebrow">MY RIVIAN DATA</p><h1>{session === 'loading' ? 'Opening your workspace…' : session === 'ending' ? 'Ending your session…' : session === 'ended' ? 'Your session has ended.' : 'Open a fresh session.'}</h1><p>{session === 'loading' ? 'Connecting to the app on this device.' : session === 'ending' ? 'Closing your workspace.' : session === 'ended' ? 'Relaunch My Rivian Data to open your workspace again.' : sessionError}</p><p className="muted">{mode === 'demo' ? 'No Rivian account is connected. This demo uses made-up vehicles and data.' : 'Account information stays in the app on this device until you disconnect or stop it.'}</p></section> : <>
          {mode === 'demo' && <div className="demo-notice"><Icon name="shield" /><p><strong>Offline demo</strong> Sample vehicles and data only. No account is connected, and nothing here can change your vehicle.</p></div>}
          {sessionError && <p className="error-message" role="alert">{sessionError}</p>}
          {mode === 'live' && account.state !== 'authenticated' ? <AccountView account={account} onChanged={accountChanged} onSessionExpired={expireSession} /> : <>
            {mode === 'live' && <div className="account-connected"><p><strong>Connected to Rivian.</strong> Readings may be delayed or unavailable while your vehicle is asleep.</p><button type="button" disabled={garageLoading} onClick={() => void disconnect()}>Disconnect account</button></div>}
            {garageLoading ? <p role="status">Loading your vehicles…</p> : vehicles.length === 0 && view !== 'developer' ? <section className="session-panel"><h1>No vehicles available</h1><p>Rivian did not return any vehicles for this account. Check your account access, then try again.</p><button className="button button-secondary" onClick={() => void accountChanged(account)}>Retry vehicle list</button></section> : view === 'developer'
              ? <DeveloperView key={accountRevision} mode={mode} operations={operations} vehicleId={vehicleId} wasm={wasm} wasmState={wasmState} onSessionExpired={expireSession} onAccountExpired={accountRequired} />
              : <OwnerView key={accountRevision} mode={mode} area={view} vehicles={vehicles} vehicleId={vehicleId} onSelectVehicle={setVehicleId} onNavigate={navigate} wasm={wasm} wasmState={wasmState} onSessionExpired={expireSession} onAccountExpired={accountRequired} />}
          </>}
          <footer className="page-footer"><span><Icon name="lock" />On your device · {mode === 'demo' ? 'Sample data only' : 'Your Rivian data'}</span><span>My Rivian Data is an independent, unofficial project.</span></footer>
        </>}
      </main>
    </div>
  </div>;
}

createRoot(document.getElementById('root')!).render(<ErrorBoundary><App /></ErrorBoundary>);
