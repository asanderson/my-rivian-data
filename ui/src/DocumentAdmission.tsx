import { useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { accountExpired, ApiError, requestWithDetails } from './api';
import { parseVariables } from './domain';

export function DocumentAdmission({ onAdmitted, onAccountExpired, onSessionExpired }: { onAdmitted: (id: string, variables: Record<string, unknown>) => void; onAccountExpired: () => void; onSessionExpired: () => void }) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState('');
  const [reply, setReply] = useState<unknown>(null);
  const mounted = useRef(true);
  const pending = useRef(false);
  useEffect(() => () => { mounted.current = false; }, []);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending.current) return;
    pending.current = true; setBusy(true); setProblem(''); setReply(null);
    const form = new FormData(event.currentTarget);
    try {
      const document = String(form.get('document') ?? '');
      if (new TextEncoder().encode(document).byteLength > 65536) throw new Error('The reviewed document exceeds the 64 KiB limit.');
      const result = await requestWithDetails<{ admitted: boolean; operation_id: string; variables: unknown }>('/api/catalog/admit', {
        endpoint: String(form.get('endpoint') ?? '').trim(), operation_name: String(form.get('operation_name') ?? '').trim(), document,
        variables: parseVariables(String(form.get('variables') ?? '{}')),
      });
      if (!mounted.current) return;
      if (result.body?.admitted !== true || typeof result.body.operation_id !== 'string') throw new Error('The native host did not admit this document.');
      const admittedVariables = parseVariables(JSON.stringify(result.body.variables));
      setReply(result.body); onAdmitted(result.body.operation_id, admittedVariables);
    } catch (error) {
      if (!mounted.current) return;
      if (accountExpired(error)) onAccountExpired();
      else if (error instanceof ApiError && error.status === 401) onSessionExpired();
      else setProblem(error instanceof Error ? error.message : 'The document could not be admitted.');
    } finally { pending.current = false; if (mounted.current) setBusy(false); }
  }
  return <details className="developer-document-admission"><summary>Import a reviewed GraphQL document</summary><p>Admission accepts only an exact document, operation name and endpoint already reviewed in this build. It identifies the matching fixed local operation; it does not send the imported document to Rivian.</p><form onSubmit={event => void submit(event)}>
    <label>Reviewed endpoint<input name="endpoint" type="url" required maxLength={512} disabled={busy} placeholder="Copy the endpoint from the operation inventory" /></label>
    <label>Reviewed operation name<input name="operation_name" required maxLength={128} disabled={busy} /></label>
    <label>Reviewed GraphQL document<textarea name="document" required maxLength={65536} disabled={busy} spellCheck={false} /></label>
    <label htmlFor="reviewed-import-variables">Import variables JSON</label><textarea id="reviewed-import-variables" name="variables" defaultValue="{}" maxLength={16384} disabled={busy} spellCheck={false} />
    <button className="button button-secondary" type="submit" disabled={busy}>{busy ? 'Checking document…' : 'Check reviewed document'}</button>
  </form>{problem && <p className="developer-error" role="alert">{problem}</p>}{reply !== null && <pre tabIndex={0} aria-label="Reviewed document admission response">{JSON.stringify(reply, null, 2)}</pre>}</details>;
}
