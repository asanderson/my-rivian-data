import { Component, useEffect, useMemo, useState } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { ApiError, clearSession, request, startSession } from './api';
import { isValidationResult, parseVariables, safeSourceUrl, validationsMatch } from './domain';
import type { Operation, ValidationResult, Vehicle, WasmModule } from './domain';
import './styles.css';

// Consume the launcher secret before rendering or making any request. It is never stored.
let launcherToken: string | null = null;
if (window.location.hash) {
  const fragment = new URLSearchParams(window.location.hash.slice(1));
  launcherToken = fragment.get('bootstrap');
  window.history.replaceState(null, '', window.location.pathname + window.location.search);
}

type IconName = 'leaf' | 'car' | 'grid' | 'code' | 'lock' | 'bolt' | 'arrow' | 'check' | 'search' | 'out' | 'shield' | 'alert';
function Icon({ name, className = '' }: { name: IconName; className?: string }) {
  const paths: Record<IconName, ReactNode> = {
    leaf: <><path d="M19 4C11 2 4 8 5 15c7 4 15-1 14-11Z" /><path d="m4 21 10-11" /></>,
    car: <><path d="m5 9 2-5h10l2 5M3 10h18v8H3zM6 18v2m12-2v2M6 13h2m8 0h2" /></>,
    grid: <><rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" /><rect x="3" y="14" width="7" height="7" rx="1" /><rect x="14" y="14" width="7" height="7" rx="1" /></>,
    code: <><path d="m8 6-6 6 6 6m8-12 6 6-6 6m-3-15-2 18" /></>,
    lock: <><rect x="5" y="10" width="14" height="11" rx="2" /><path d="M8 10V7a4 4 0 0 1 8 0v3m-4 5v2" /></>,
    bolt: <path d="m13 2-9 12h7l-1 8 10-13h-7z" />,
    arrow: <path d="M4 12h16m-6-6 6 6-6 6" />,
    check: <path d="m5 12 4 4L19 6" />,
    search: <><circle cx="10" cy="10" r="7" /><path d="m15 15 6 6" /></>,
    out: <><path d="M10 4H4v16h6m4-13 5 5-5 5m-5-5h10" /></>,
    shield: <><path d="m12 2 9 4v7c0 5-9 9-9 9S3 18 3 13V6z" /><path d="m8 12 3 3 5-6" /></>,
    alert: <><path d="m12 3 10 18H2zM12 9v5m0 3v.1" /></>,
  };
  return <svg className={`icon ${className}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}

class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(_error: Error, _info: ErrorInfo) { /* Never log request data or session material. */ }
  render() {
    if (this.state.failed) return <main className="fatal"><Icon name="alert" /><h1>The interface could not continue.</h1><p>Close this page and relaunch My Rivian Data to start a fresh session.</p></main>;
    return this.props.children;
  }
}

function VehicleCard({ vehicle, index }: { vehicle: Vehicle; index: number }) {
  return <article className={`vehicle-card vehicle-tone-${index % 2}`} aria-labelledby={`vehicle-${vehicle.id}`}>
    <div className="vehicle-top"><span className="model-tag">{vehicle.model}</span><span className="sample-label">SAMPLE VEHICLE</span></div>
    <div className="vehicle-title"><div><h3 id={`vehicle-${vehicle.id}`}>{vehicle.name}</h3><p>{vehicle.model_year} · {vehicle.model}</p></div><Icon name="car" className="vehicle-icon" /></div>
    <div className="vehicle-metrics"><div><span className="metric-number">{vehicle.battery_percent}<small>%</small></span><span className="metric-label">Battery</span></div><div><span className="metric-number">{Math.round(vehicle.estimated_range_km)}<small>km</small></span><span className="metric-label">Estimated range</span></div></div>
    <label className="sr-only" htmlFor={`battery-${vehicle.id}`}>{vehicle.name} battery charge</label>
    <progress id={`battery-${vehicle.id}`} value={vehicle.battery_percent} max="100" />
    <div className="vehicle-foot"><span><Icon name="lock" />{vehicle.locked ? 'Locked' : 'Unlocked'}</span><span><Icon name="bolt" />{vehicle.charging_state.replaceAll('_', ' ')}</span></div>
  </article>;
}

interface ValidationPair { native: ValidationResult; wasm: ValidationResult; parity: boolean }
const errorMessage = (error: unknown) => error instanceof Error ? error.message : 'Something went wrong. Please try again.';

function App() {
  const [session, setSession] = useState<'loading' | 'ready' | 'ended' | 'error'>('loading');
  const [sessionError, setSessionError] = useState('');
  const [operations, setOperations] = useState<Operation[]>([]);
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [selectedId, setSelectedId] = useState('vehicle-state');
  const [variables, setVariables] = useState('{}');
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState('all');
  const [wasm, setWasm] = useState<WasmModule | null>(null);
  const [wasmState, setWasmState] = useState<'loading' | 'ready' | 'failed'>('loading');
  const [busy, setBusy] = useState<'validate' | 'execute' | 'logout' | null>(null);
  const [validation, setValidation] = useState<ValidationPair | null>(null);
  const [problem, setProblem] = useState('');
  const [response, setResponse] = useState('');
  const [responseOperation, setResponseOperation] = useState('');
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    let mounted = true;
    const token = launcherToken;
    launcherToken = null;
    void (async () => {
      try {
        await startSession(token);
        const [catalog, garage] = await Promise.all([
          request<Operation[]>('/api/catalog'), request<Vehicle[]>('/api/vehicles'),
        ]);
        if (!Array.isArray(catalog) || !Array.isArray(garage) || catalog.length > 256 || garage.length > 64) throw new Error('The local service returned an invalid demo catalog.');
        if (!mounted) return;
        setOperations(catalog);
        setVehicles(garage);
        const initial = catalog.find(op => op.id === 'vehicle-state') ?? catalog[0];
        if (initial) { setSelectedId(initial.id); setVariables(JSON.stringify(initial.example_variables, null, 2)); }
        setSession('ready');
        try {
          const wasmUrl = '/wasm/rivian_wasm.js';
          const module = await import(/* @vite-ignore */ wasmUrl) as WasmModule;
          await module.default();
          if (typeof module.validate_request_json !== 'function') throw new Error('Validator export is unavailable.');
          if (mounted) { setWasm(module); setWasmState('ready'); }
        } catch { if (mounted) setWasmState('failed'); }
      } catch (error) {
        clearSession();
        if (mounted) {
          setSession('error');
          setSessionError(error instanceof ApiError && error.status === 401
            ? 'This browser session is missing or has expired. Relaunch My Rivian Data and use the page it opens.'
            : errorMessage(error));
        }
      }
    })();
    return () => { mounted = false; };
  }, []);

  const selected = operations.find(op => op.id === selectedId);
  const visibleOperations = useMemo(() => operations.filter(op => {
    const matchesFilter = filter === 'all' || (filter === 'query' ? op.kind === 'query' : op.kind !== 'query');
    return matchesFilter && `${op.label} ${op.description} ${op.family}`.toLowerCase().includes(search.toLowerCase());
  }), [operations, search, filter]);
  const demoCount = operations.filter(op => op.status === 'demo').length;

  function handleRequestError(error: unknown) {
    if (error instanceof ApiError && error.status === 401) {
      clearSession(); setSession('error'); setSessionError('Your session has expired. Relaunch My Rivian Data to continue.');
    } else setProblem(errorMessage(error));
  }

  function selectOperation(operation: Operation) {
    setSelectedId(operation.id);
    setVariables(JSON.stringify(operation.example_variables, null, 2));
    setValidation(null); setProblem(''); setResponse(''); setResponseOperation('');
  }

  async function check(): Promise<ValidationPair> {
    if (!selected || !wasm) throw new Error('The WebAssembly validator must be ready before a request can run.');
    const parsed = parseVariables(variables);
    const wasmResult: unknown = JSON.parse(wasm.validate_request_json(selected.id, JSON.stringify(parsed)));
    if (!isValidationResult(wasmResult)) throw new Error('The WebAssembly validator returned an unexpected result.');
    const nativeResult: unknown = await request<ValidationResult>('/api/validate', { operation_id: selected.id, variables: parsed });
    if (!isValidationResult(nativeResult)) throw new Error('The native validator returned an unexpected result.');
    const pair = { native: nativeResult, wasm: wasmResult, parity: validationsMatch(nativeResult, wasmResult) };
    setValidation(pair);
    return pair;
  }

  async function validate() {
    setBusy('validate'); setProblem(''); setValidation(null);
    try { await check(); } catch (error) { handleRequestError(error); }
    finally { setBusy(null); }
  }

  async function execute() {
    if (!selected || selected.status !== 'demo') return;
    setBusy('execute'); setProblem(''); setResponse(''); setValidation(null);
    const started = performance.now();
    try {
      const pair = await check();
      if (!pair.parity) throw new Error('The validators disagree. This request was not run.');
      if (!pair.native.valid || !pair.wasm.valid) throw new Error('Correct the validation issues before running this request.');
      const result = await request<{ data: unknown; mode: string }>('/api/execute', { operation_id: selected.id, variables: parseVariables(variables) });
      if (result.mode !== 'demo') throw new Error('The local service returned a response outside offline demo mode.');
      setResponse(JSON.stringify(result.data, null, 2));
      setResponseOperation(selected.label);
      setElapsed(Math.round(performance.now() - started));
    } catch (error) { handleRequestError(error); }
    finally { setBusy(null); }
  }

  async function logout() {
    setBusy('logout'); setProblem('');
    try { await request<void>('/api/logout', {}); clearSession(); setSession('ended'); setVehicles([]); setOperations([]); setResponse(''); }
    catch (error) { handleRequestError(error); }
    finally { setBusy(null); }
  }

  const sourceUrl = selected ? safeSourceUrl(selected.source_url) : undefined;
  const disabled = busy !== null;
  return <div className="app-shell">
    <a className="skip-link" href="#main">Skip to main content</a>
    <aside className="sidebar" aria-label="Application navigation">
      <a className="brand" href="#main" aria-label="My Rivian Data home"><span className="brand-mark"><Icon name="leaf" /></span><span>My Rivian <strong>Data</strong><small>YOUR VEHICLES. YOUR DATA.</small></span></a>
      <div className="nav-caption">WORKSPACE</div>
      <nav><a className="nav-link active" href="#garage"><Icon name="grid" />Overview<span className="nav-dot" /></a><a className="nav-link" href="#explorer"><Icon name="code" />API explorer</a></nav>
      <div className="sidebar-note"><span className="sidebar-note-icon"><Icon name="shield" /></span><h2>It stays local.</h2><p>This prototype runs on your device and uses synthetic vehicle data.</p><span className="sidebar-status"><span className="status-dot" />Offline demo</span></div>
      <div className="sidebar-footer">INDEPENDENT COMMUNITY PROJECT<br />Not affiliated with Rivian</div>
    </aside>

    <div className="main-shell">
      <header className="topbar"><span className="breadcrumb">Workspace <span>/</span> Overview</span><div className="topbar-right"><span className="mode-badge"><span className="status-dot" />Offline demo</span>{session === 'ready' && <button className="end-session" aria-label="End session" onClick={() => void logout()} disabled={disabled}><Icon name="out" /><span>{busy === 'logout' ? 'Ending…' : 'End session'}</span></button>}</div></header>
      <main id="main" tabIndex={-1}>
        {session !== 'ready' ? <section className="session-panel" aria-live="polite"><span className="intro-icon"><Icon name={session === 'loading' ? 'leaf' : 'lock'} /></span><p className="eyebrow">MY RIVIAN DATA</p><h1>{session === 'loading' ? 'Opening your local workspace…' : session === 'ended' ? 'Your session has ended.' : 'Open a fresh local session.'}</h1><p>{session === 'loading' ? 'Connecting to the app on this device.' : session === 'ended' ? 'Relaunch My Rivian Data to open the offline demo again.' : sessionError}</p><p className="muted">No Rivian account is connected. Real authentication is planned for a later milestone.</p></section> : <>
          <section className="intro" aria-labelledby="page-title"><div><p className="eyebrow">BECAUSE IT’S YOUR DATA</p><h1 id="page-title">Your vehicles. Your data.</h1><p>Built for easy access to the data from the Rivian vehicles you own.</p></div><span className="prototype-pill">Phase 1 · Prototype</span></section>
          <div className="demo-notice"><Icon name="shield" /><p><strong>Take a look around. This is an offline demo.</strong> Every vehicle and response is synthetic. No credentials are collected, and vehicle controls are disabled.</p></div>

          <section id="garage" className="garage" aria-labelledby="garage-title"><div className="section-heading"><div><h2 id="garage-title">Your garage <span className="count-pill">{vehicles.length}</span></h2><p>Sample vehicles to explore the experience.</p></div><span className="quiet-meta">DEMO DATA · NOT LIVE</span></div><div className="vehicle-grid">{vehicles.map((vehicle, index) => <VehicleCard key={vehicle.id} vehicle={vehicle} index={index} />)}</div></section>

          <section id="explorer" className="explorer" aria-labelledby="explorer-title"><div className="section-heading"><div><h2 id="explorer-title">Explore the API</h2><p>Choose a request, inspect its inputs, and run a safe local example.</p></div><span className="quiet-meta">{demoCount} DEMO REQUESTS</span></div>
            <div className="explorer-frame"><aside className="catalog" aria-label="Operation catalog"><div className="catalog-search"><Icon name="search" /><input type="search" aria-label="Search API requests" placeholder="Find a request…" value={search} maxLength={160} onChange={event => setSearch(event.target.value)} /></div><div className="catalog-filters" role="group" aria-label="Filter requests">{[['all', 'All'], ['query', 'Queries'], ['other', 'Controls & more']].map(([value, label]) => <button key={value} type="button" aria-pressed={filter === value} className={filter === value ? 'selected' : ''} onClick={() => setFilter(value)}>{label}</button>)}</div><div className="operation-list">{visibleOperations.map(operation => <button key={operation.id} className={`operation-item ${selectedId === operation.id ? 'selected' : ''}`} type="button" aria-pressed={selectedId === operation.id} onClick={() => selectOperation(operation)} disabled={disabled}><span className="operation-symbol"><Icon name={operation.kind === 'query' ? 'code' : operation.status === 'blocked' ? 'lock' : 'bolt'} /></span><span className="operation-copy"><strong>{operation.label}</strong><small>{operation.family}</small></span><span className={`operation-status status-${operation.status}`}>{operation.status === 'demo' ? 'Demo' : operation.status === 'blocked' ? 'Blocked' : 'Planned'}</span></button>)}{visibleOperations.length === 0 && <p className="no-results">No matching requests.<br />Try another search or filter.</p>}</div><p className="catalog-foot">A starter catalog of local examples.<br />Full Rivian API coverage is planned.</p></aside>

              <div className="request-workspace">{selected ? <><div className="request-heading"><div><span className="request-family">{selected.family} <span>/</span> {selected.kind}</span><h3>{selected.label}</h3></div><span className={`request-status status-${selected.status}`}>{selected.status === 'demo' ? 'Safe demo query' : selected.status === 'blocked' ? 'Control disabled' : 'Coming later'}</span></div><p className="request-description">{selected.description}</p>
                {selected.status !== 'demo' && <div className="inline-notice"><Icon name="lock" /><span>{selected.status === 'blocked' ? 'This vehicle control is blocked in the prototype. No command can be sent.' : 'This operation is included for discovery. Live streaming is not implemented yet.'}</span></div>}
                <div className="request-inputs"><div className="editor-heading"><label htmlFor="variables">Request variables</label><button className="text-button" type="button" disabled={disabled} onClick={() => { setVariables(JSON.stringify(selected.example_variables, null, 2)); setValidation(null); setProblem(''); }}>Reset example</button></div><textarea id="variables" className="json-editor" value={variables} onChange={event => { setVariables(event.target.value); setValidation(null); setProblem(''); }} maxLength={16_384} spellCheck={false} autoCapitalize="off" autoCorrect="off" aria-describedby="variables-help request-problem" aria-invalid={!!problem || (validation !== null && !validation.native.valid)} disabled={disabled} /><div id="variables-help" className="editor-help"><span>JSON object · 16 KiB maximum</span><span className={`wasm-state ${wasmState}`}><span className="status-dot" />{wasmState === 'ready' ? 'Rust/WASM ready' : wasmState === 'failed' ? 'Rust/WASM unavailable' : 'Loading Rust/WASM…'}</span></div></div>
                {wasmState === 'failed' && <div className="error-message" role="alert">The WebAssembly validator could not load. Requests are paused. Rebuild the app’s WASM assets and relaunch to try again.</div>}
                <div className="request-actions"><button className="button button-secondary" type="button" onClick={() => void validate()} disabled={disabled || wasmState !== 'ready'}>{busy === 'validate' ? 'Validating…' : 'Validate inputs'}</button><button className="button button-primary" type="button" onClick={() => void execute()} disabled={disabled || wasmState !== 'ready' || selected.status !== 'demo'}>{busy === 'execute' ? 'Running…' : 'Run demo request'}<Icon name="arrow" /></button></div>
                <div className="validation-area" aria-live="polite">{validation && <><div className={`validation-summary ${validation.parity && validation.native.valid ? 'valid' : 'invalid'}`}><Icon name={validation.parity && validation.native.valid ? 'check' : 'alert'} /><span><strong>{validation.parity ? 'Native and WASM results match' : 'Native and WASM results differ'}</strong><small>{validation.native.valid && validation.wasm.valid ? 'Both validators accept these inputs.' : 'One or both validators rejected these inputs.'}</small></span></div>{(!validation.native.valid || !validation.parity) && <div className="validation-issues"><div><strong>Native Rust</strong>{validation.native.issues.length ? <ul>{validation.native.issues.map((issue, i) => <li key={i}><code>{issue.field}</code>: {issue.message}</li>)}</ul> : <p>No issues.</p>}</div><div><strong>Rust/WASM</strong>{validation.wasm.issues.length ? <ul>{validation.wasm.issues.map((issue, i) => <li key={i}><code>{issue.field}</code>: {issue.message}</li>)}</ul> : <p>No issues.</p>}</div></div>}</>}</div>
                <div id="request-problem" className={problem ? 'error-message' : 'empty-error'} role="alert">{problem}</div>
                <div className="response-panel"><div className="response-heading"><h4>Response</h4>{response && <span><span className="status-dot" />Synthetic · {elapsed} ms</span>}</div>{response ? <><p className="sr-only" role="status">{responseOperation} completed with a synthetic response.</p><pre className="response-json" tabIndex={0} aria-label={`${responseOperation} synthetic response`}>{response}</pre></> : <div className="response-empty"><Icon name="code" /><p>Your response will appear here.</p><span>Demo requests only read synthetic data.</span></div>}</div>
                <div className="request-footer"><span>Local operation: <code>{selected.id}</code></span>{sourceUrl && <a href={sourceUrl} target="_blank" rel="noreferrer noopener">Community API reference ↗<span className="sr-only"> (opens in a new tab)</span></a>}</div></> : <p className="no-results">No operations are available in this build.</p>}</div>
            </div>
          </section>
          <footer className="page-footer"><span><Icon name="lock" />Local session · No cloud connection</span><span>My Rivian Data is an independent, unofficial project.</span></footer>
        </>}
      </main>
    </div>
  </div>;
}

createRoot(document.getElementById('root')!).render(<ErrorBoundary><App /></ErrorBoundary>);
