import { useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { ApiError, readAccountStatus, request } from './api';
import type { AccountStatus } from './api';
import { Icon } from './Icon';

export function AccountView({ account, onChanged, onSessionExpired }: {
  account: AccountStatus; onChanged: (status: AccountStatus) => Promise<void>; onSessionExpired: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState('');
  const form = useRef<HTMLFormElement>(null);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const mfa = account.state === 'mfa_required';
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy) return;
    const values = new FormData(event.currentTarget);
    const body = mfa ? { code: String(values.get('code') ?? '').trim() }
      : { email: String(values.get('email') ?? '').trim(), password: String(values.get('password') ?? '') };
    // Clear sensitive DOM inputs immediately; neither credentials nor OTP enter React state or storage.
    const sensitive = form.current?.elements.namedItem(mfa ? 'code' : 'password') as HTMLInputElement | null;
    if (sensitive) sensitive.value = '';
    setBusy(true); setProblem('');
    try {
      const status = readAccountStatus(await request('/api/account/' + (mfa ? 'otp' : 'login'), body));
      if (!mounted.current) return;
      form.current?.reset();
      await onChanged(status);
    } catch (error) {
      if (!mounted.current) return;
      if (error instanceof ApiError && error.code === 'mfa_expired') { await onChanged({ state: 'signed_out' }); setProblem(error.message); }
      else if (error instanceof ApiError && ['session_required', 'unauthorized'].includes(error.code)) onSessionExpired();
      else setProblem(error instanceof Error ? error.message : 'Sign-in could not be completed. Please try again.');
    } finally { if (mounted.current) setBusy(false); }
  }
  async function startOver() {
    if (busy) return;
    setBusy(true); setProblem('');
    try { await request('/api/account/logout', {}); if (mounted.current) await onChanged({ state: 'signed_out' }); }
    catch (error) { if (mounted.current) setProblem(error instanceof Error ? error.message : 'Please relaunch the app.'); }
    finally { if (mounted.current) setBusy(false); }
  }
  return <section className="session-panel account-panel" aria-labelledby="account-title">
    <Icon name="shield" /><p className="eyebrow">YOUR VEHICLES. YOUR DATA.</p>
    <h1 id="account-title">{mfa ? 'Verify your sign-in' : 'Connect your Rivian account'}</h1>
    <p>{mfa ? 'Enter the verification code from Rivian. Check your authenticator or the destination Rivian uses for your account.' : 'Sign in with the email and password you use at rivian.com to view your vehicles and their available data.'}</p>
    {mfa && account.channel && <p className="account-channel">Verification method: {account.channel}</p>}
    <form ref={form} onSubmit={event => void submit(event)} className="account-form" key={account.state}>
      {mfa ? <label>Verification code<input name="code" type="text" inputMode="numeric" autoComplete="one-time-code" autoFocus required minLength={6} maxLength={8} pattern="[0-9]{6,8}" disabled={busy} /></label> : <>
        <label>Email address<input name="email" type="email" autoComplete="username" required maxLength={254} disabled={busy} /></label>
        <label>Password<input name="password" type="password" autoComplete="current-password" required maxLength={1024} disabled={busy} /></label>
      </>}
      {problem && <p className="error-message" role="alert">{problem}</p>}
      <button className="button button-primary" disabled={busy} type="submit">{busy ? 'Connecting…' : mfa ? 'Verify and continue' : 'Sign in'}</button>
      {mfa && <button className="button button-secondary" disabled={busy} type="button" onClick={() => void startOver()}>Use a different account</button>}
    </form>
    <p className="muted">Your password is sent through the app on this device to Rivian over an encrypted connection. The app does not save it. Disconnecting or stopping the app clears your sign-in. You may need to sign in again after inactivity or when Rivian expires your session. This is an independent, unofficial project.</p>
  </section>;
}
