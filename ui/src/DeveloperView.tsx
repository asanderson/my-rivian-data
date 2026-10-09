import { useEffect, useMemo, useRef, useState } from 'react';
import { accountExpired, ApiError, requestWithDetails } from './api';
import type { AppMode, ApiResponse } from './api';
import { parseVariables, safeSourceUrl } from './domain';
import type { Operation, ValidationResult, WasmModule } from './domain';
import { Icon } from './Icon';
import { validateOperation } from './operations';
import './developer.css';

interface DeveloperViewProps {
  mode: AppMode;
  operations: Operation[];
  vehicleId: string;
  wasm: WasmModule | null;
  wasmState: 'loading' | 'ready' | 'failed';
  onSessionExpired: () => void;
  onAccountExpired: () => void;
}

interface ValidationPair { native: ValidationResult; wasm: ValidationResult; parity: boolean }
interface Parameter { name: string; type: string; required: boolean; help: string }

function parameters(operation: Operation, mode: AppMode): Parameter[] {
  const items: Parameter[] = operation.requires_vehicle ? [{ name: 'vehicle_id', type: 'string', required: true, help: mode === 'demo' ? 'An authorized sample vehicle ID from the demo garage.' : 'A vehicle ID returned for the signed-in Rivian account.' }] : [];
  if (operation.id === 'charging-history') items.push({ name: 'limit', type: 'integer', required: false, help: mode === 'demo' ? '1–100 sessions; defaults to 10. Only two synthetic sessions exist.' : '1–100 sessions; defaults to 10. Only sessions associated with the selected vehicle are returned.' });
  if (operation.id === 'set-charge-limit') items.push({ name: 'limit_percent', type: 'integer', required: true, help: '50–100 percent. Schema example only; this command is blocked.' });
  return items;
}

function exampleVariables(operation: Operation, vehicleId: string): string {
  return JSON.stringify({ ...operation.example_variables, ...(operation.requires_vehicle ? { vehicle_id: vehicleId } : {}) }, null, 2);
}

const operationStatus = (operation: Operation) => operation.status === 'live' ? 'Live query' : operation.status === 'demo' ? 'Demo query' : operation.status === 'blocked' ? 'Blocked command' : 'Planned stream';

export function DeveloperView({ mode, operations, vehicleId, wasm, wasmState, onSessionExpired, onAccountExpired }: DeveloperViewProps) {
  const [selectedId, setSelectedId] = useState('vehicle-state');
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState<'all' | 'demo' | 'other'>('all');
  const selected = operations.find(operation => operation.id === selectedId) ?? operations[0];
  const groups = useMemo(() => {
    const found = new Map<string, Operation[]>();
    for (const operation of operations) {
      if (filter === 'demo' && !['demo', 'live'].includes(operation.status)) continue;
      if (filter === 'other' && ['demo', 'live'].includes(operation.status)) continue;
      if (!`${operation.id} ${operation.label} ${operation.family} ${operation.description}`.toLowerCase().includes(search.toLowerCase())) continue;
      found.set(operation.family, [...(found.get(operation.family) ?? []), operation]);
    }
    return [...found.entries()];
  }, [operations, search, filter]);

  return <section className="developer-view" aria-labelledby="developer-title">
    <div className="developer-intro"><div><p className="developer-eyebrow">TECHNICAL WORKSPACE</p><h1 id="developer-title">API explorer</h1><p>Inspect operation inputs, compare validators, and examine complete local request and response bodies.</p></div><span className="developer-count">{operations.length} local operations</span></div>
    <div className="developer-notice"><Icon name="code" /><p>{mode === 'demo' ? <><strong>Local demo adapter, not the upstream Rivian API.</strong> These operation IDs and parameter descriptions belong to the local app. All responses are synthetic; commands and live streams cannot run.</> : <><strong>Native Rivian API adapter.</strong> Requests use the fixed operations below and your native account session. Responses include the redacted upstream body when supplied by the adapter. Account tokens and headers are never shown. Unavailable operations cannot run.</>}</p></div>
    <div className="developer-layout">
      <aside className="developer-catalog" aria-label="Developer operation catalog">
        <label className="developer-search"><Icon name="search" /><input type="search" aria-label="Search API requests" placeholder="Find an operation…" value={search} maxLength={160} onChange={event => setSearch(event.target.value)} /></label>
        <div className="developer-filters" role="group" aria-label="Filter API requests">{([['all', 'All'], ['demo', mode === 'demo' ? 'Demo queries' : 'Live queries'], ['other', 'Unavailable']] as const).map(([value, label]) => <button key={value} type="button" aria-pressed={filter === value} onClick={() => setFilter(value)}>{label}</button>)}</div>
        <div className="developer-groups">{groups.map(([family, group]) => <section className="developer-group" key={family} aria-label={`${family} operations`}><h3>{family}</h3>{group.map(operation => <button key={operation.id} type="button" className="developer-operation" aria-pressed={operation.id === selected?.id} onClick={() => setSelectedId(operation.id)}><span className="developer-operation-name">{operation.label}</span><code>{operation.id}</code><span className={`developer-badge developer-${operation.status}`}>{operationStatus(operation)}</span></button>)}</section>)}{!groups.length && <p className="developer-empty">No matching operations. Try another search or filter.</p>}</div>
        <p className="developer-catalog-note">{mode === 'demo' ? 'A catalog of local examples. Full Rivian API coverage is not available in demo mode.' : 'The catalog documents supported requests and explicit gaps. This unofficial API can change without notice.'}</p>
      </aside>
      {selected ? <RequestWorkbench key={`${mode}:${vehicleId}:${selected.id}`} mode={mode} operation={selected} vehicleId={vehicleId} wasm={wasm} wasmState={wasmState} onSessionExpired={onSessionExpired} onAccountExpired={onAccountExpired} /> : <p className="developer-empty">No operations are available in this build.</p>}
    </div>
  </section>;
}

function RequestWorkbench({ mode, operation, vehicleId, wasm, wasmState, onSessionExpired, onAccountExpired }: Omit<DeveloperViewProps, 'operations'> & { operation: Operation }) {
  const [variables, setVariables] = useState(() => exampleVariables(operation, vehicleId));
  const [busy, setBusy] = useState<'validate' | 'execute' | null>(null);
  const [validation, setValidation] = useState<ValidationPair | null>(null);
  const [problem, setProblem] = useState('');
  const [response, setResponse] = useState<ApiResponse<unknown> | null>(null);
  const [formatResponse, setFormatResponse] = useState(true);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const sourceUrl = safeSourceUrl(operation.source_url);
  const parameterRows = parameters(operation, mode);
  const executable = operation.status === mode && operation.kind === 'query';
  const requestPreview = useMemo(() => {
    try { return JSON.stringify({ operation_id: operation.id, variables: parseVariables(variables) }, null, 2); }
    catch { return null; }
  }, [operation.id, variables]);

  function changeVariables(text: string) {
    setVariables(text); setValidation(null); setProblem(''); setResponse(null);
  }

  function handleError(error: unknown) {
    if (!mounted.current) return;
    if (accountExpired(error)) { setResponse(null); setValidation(null); onAccountExpired(); }
    else if (error instanceof ApiError && error.status === 401) {
      setResponse(null); setValidation(null); onSessionExpired();
    } else setProblem(error instanceof Error ? error.message : 'The local request could not be completed.');
  }

  async function run(action: 'validate' | 'execute') {
    if (busy || !wasm || wasmState !== 'ready') return;
    if (action === 'execute' && !executable) return;
    setBusy(action); setProblem(''); setResponse(null); setValidation(null);
    let requestingResponse = false;
    try {
      const parsed = parseVariables(variables);
      const pair = await validateOperation(wasm, operation.id, parsed, mode);
      if (!mounted.current) return;
      setValidation(pair);
      if (action === 'validate') return;
      if (!pair.parity) throw new Error('Native and WASM validators disagree. Execution was stopped.');
      if (!pair.native.valid || !pair.wasm.valid) throw new Error('Correct the validation issues before running this request.');
      requestingResponse = true;
      const result = await requestWithDetails<{ mode: string; data: unknown }>('/api/execute', { operation_id: operation.id, variables: parsed });
      if (!mounted.current) return;
      if (!result.body || result.body.mode !== mode) throw new Error('The local service returned a response outside the current connection mode.');
      setResponse(result);
    } catch (error) {
      if (!mounted.current) return;
      if (requestingResponse && error instanceof ApiError && error.status !== 401 && error.response) setResponse(error.response);
      handleError(error);
    } finally { if (mounted.current) setBusy(null); }
  }

  return <div className="developer-workbench">
    <div className="developer-operation-heading"><div><p className="developer-eyebrow">{operation.family} / {operation.kind}</p><h3>{operation.label}</h3></div><span className={`developer-badge developer-${operation.status}`}>{operationStatus(operation)}</span></div>
    <p className="developer-description">{operation.description}</p>
    <div className="developer-endpoint"><span>POST</span><code>/api/execute</code><small>Local host</small></div>
    <dl className="developer-facts"><div><dt>Operation ID</dt><dd><code>{operation.id}</code></dd></div><div><dt>Content type</dt><dd><code>application/json</code></dd></div>{operation.upstream_operation && <div><dt>Upstream operation</dt><dd><code>{operation.upstream_operation}</code></dd></div>}</dl>
    {!executable && <div className="developer-unavailable"><Icon name="lock" /><p>{operation.status === 'blocked' ? 'Execution is disabled for this vehicle command. No command is sent.' : 'This operation is a catalog entry only. Its transport is not implemented, and validation rejects execution.'}</p></div>}

    {operation.upstream_document && <details className="developer-request-preview"><summary>Upstream GraphQL document</summary><p>The native adapter constructs upstream variables from the validated local inputs and your account. Credentials are never part of this document.</p><pre tabIndex={0}>{operation.upstream_document}</pre>{operation.endpoint && <p>{operation.endpoint}</p>}</details>}
    <section className="developer-parameters" aria-labelledby="developer-parameters-title"><h4 id="developer-parameters-title">Variables</h4><p>Local schema summary. Unknown fields are rejected.</p>{parameterRows.length ? <div className="developer-table-wrap"><table><thead><tr><th scope="col">Name</th><th scope="col">Type</th><th scope="col">Requirement</th><th scope="col">Description</th></tr></thead><tbody>{parameterRows.map(parameter => <tr key={parameter.name}><th scope="row"><code>{parameter.name}</code></th><td>{parameter.type}</td><td>{parameter.required ? 'Required' : 'Optional'}</td><td>{parameter.help}</td></tr>)}</tbody></table></div> : <p className="developer-no-parameters">No variables. Send an empty JSON object: <code>{'{}'}</code>.</p>}</section>
    <div className="developer-editor-heading"><label htmlFor="developer-variables">Request variables JSON</label><button type="button" disabled={!!busy} onClick={() => changeVariables(exampleVariables(operation, vehicleId))}>Reset example</button></div>
    <textarea id="developer-variables" className="developer-json-editor" value={variables} onChange={event => changeVariables(event.target.value)} spellCheck={false} autoCapitalize="off" autoCorrect="off" maxLength={16_384} disabled={!!busy} aria-describedby="developer-variables-help developer-problem" aria-invalid={!!problem || (validation !== null && !validation.native.valid)} />
    <p id="developer-variables-help" className="developer-editor-help">JSON object · 16 KiB maximum · depth 8 · 256 nodes</p>
    <details className="developer-request-preview" open><summary>Complete request body</summary>{requestPreview ? <pre tabIndex={0} aria-label="Complete request body">{requestPreview}</pre> : <p>Enter a valid JSON object within the size limit to preview the request.</p>}<p>Session and CSRF credentials are attached internally and are not displayed.</p></details>
    <div className="developer-actions"><span className={`developer-validator developer-${wasmState}`}><span className="status-dot" />{wasmState === 'ready' ? 'Rust/WASM ready' : wasmState === 'loading' ? 'Loading Rust/WASM…' : 'Rust/WASM unavailable'}</span><button type="button" className="button button-secondary" disabled={!!busy || wasmState !== 'ready'} onClick={() => void run('validate')}>{busy === 'validate' ? 'Validating…' : 'Validate inputs'}</button><button type="button" className="button button-primary" disabled={!!busy || wasmState !== 'ready' || !executable} onClick={() => void run('execute')}>{busy === 'execute' ? 'Running…' : mode === 'demo' ? 'Run demo request' : 'Run request'}<Icon name="arrow" /></button></div>
    <p className="developer-authority">Validation compares Rust/WASM with <code>POST /api/validate</code>. The native host revalidates every execution and remains authoritative.</p>
    {wasmState === 'failed' && <p className="developer-error" role="alert">The WASM validator did not load. Rebuild the WASM assets and relaunch before using the explorer.</p>}
    <div className="developer-validation" aria-live="polite">{validation && <><p className={`developer-validation-summary ${validation.parity && validation.native.valid ? 'developer-valid' : 'developer-invalid'}`}><Icon name={validation.parity && validation.native.valid ? 'check' : 'alert'} /><span><strong>{validation.parity ? 'Native and WASM results match' : 'Native and WASM results differ'}</strong><small>{validation.native.valid && validation.wasm.valid ? 'Both validators accept these inputs.' : 'One or both validators rejected these inputs.'}</small></span></p>{(!validation.native.valid || !validation.wasm.valid || !validation.parity) && <div className="developer-validation-issues">{([['Native Rust', validation.native], ['Rust/WASM', validation.wasm]] as const).map(([label, result]) => <div key={label}><h4>{label}</h4>{result.issues.length ? <ul>{result.issues.map((issue, index) => <li key={index}><code>{issue.field}</code>: {issue.message}</li>)}</ul> : <p>No issues.</p>}</div>)}</div>}</>}</div>
    <div id="developer-problem" className={problem ? 'developer-error' : 'developer-no-error'} role="alert">{problem}</div>
    <section className="developer-response" aria-labelledby="developer-response-title"><div className="developer-response-heading"><h4 id="developer-response-title">Response</h4>{response && <span className={response.status < 400 ? 'developer-http-ok' : 'developer-http-error'}>HTTP {response.status} · {response.durationMs} ms</span>}</div>{response ? <><div className="developer-response-tools"><span>POST /api/execute · complete body</span><label><input type="checkbox" checked={formatResponse} onChange={event => setFormatResponse(event.target.checked)} />Format JSON</label></div><p className="sr-only" role="status">{operation.label} returned HTTP {response.status}.</p><pre className="developer-response-json" tabIndex={0} aria-label={`${operation.label} raw response`}>{formatResponse ? JSON.stringify(response.body, null, 2) : response.rawBody}</pre></> : <div className="developer-response-empty"><Icon name="code" /><p>Run {mode === 'demo' ? 'a demo' : 'a live'} request to inspect its response.</p><span>The complete JSON envelope, including <code>mode</code>, appears here.</span></div>}</section>
    <div className="developer-footer"><span>Elapsed time covers the execute request and bounded response read.</span>{sourceUrl && <a href={sourceUrl} target="_blank" rel="noreferrer noopener">Community API reference ↗<span className="sr-only"> (opens in a new tab)</span></a>}</div>
  </div>;
}
