import { useEffect, useRef, useState } from 'react';
import { accountExpired, ApiError, request } from './api';
import { readVaultStatus, vaultMessage } from './account-data';
import type { VaultStatus } from './account-data';

export function RememberedSignIn({ onSessionExpired, onAccountExpired }: { onSessionExpired: () => void; onAccountExpired: () => void }) {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState('');
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    void request('/api/account/vault').then(value => { if (mounted.current) setStatus(readVaultStatus(value)); }).catch(() => { if (mounted.current) setProblem('Remembered sign-in status is unavailable.'); });
    return () => { mounted.current = false; };
  }, []);
  async function change(remember: boolean) {
    if (busy) return;
    setBusy(true); setProblem('');
    try { const next = readVaultStatus(await request('/api/account/remember', { remember })); if (mounted.current) setStatus(next); }
    catch (error) { if (mounted.current) { if (accountExpired(error)) onAccountExpired(); else if (error instanceof ApiError && error.status === 401) onSessionExpired(); else setProblem(error instanceof Error ? error.message : 'Remembered sign-in could not be changed.'); } }
    finally { if (mounted.current) setBusy(false); }
  }
  return <div className="remembered-sign-in"><label className="account-remember"><input type="checkbox" checked={status?.remembering ?? false} disabled={busy || !status || !status.available && !status.remembering} onChange={event => void change(event.target.checked)} />Remember sign-in on this device</label><p role="status">{status ? vaultMessage(status) : 'Checking protected credential storage…'}</p>{problem && <p role="alert">{problem}</p>}</div>;
}
