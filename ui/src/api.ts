export interface ApiResponse<T> {
  body: T;
  rawBody: string;
  status: number;
  durationMs: number;
}

export class ApiError extends Error {
  status: number;
  code: string;
  response?: ApiResponse<unknown>;
  constructor(status: number, code: string, message: string, response?: ApiResponse<unknown>) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
    this.response = response;
  }
}

export type AppMode = 'demo' | 'live';
export interface LocalSession { csrf_token: string; mode: AppMode; app_capability?: string; expires_in_seconds?: number }
const CAPABILITY_KEY = 'my-rivian-data-tab-capability';
let csrfToken: string | undefined;
let appCapability: string | undefined;
// This is an origin-bound local app capability, never a Rivian credential or token.
function savedCapability(): string | undefined {
  try { return window.sessionStorage?.getItem(CAPABILITY_KEY) ?? undefined; } catch { return undefined; }
}
export function clearSession(): void {
  csrfToken = undefined; appCapability = undefined;
  try { window.sessionStorage?.removeItem(CAPABILITY_KEY); } catch { /* Storage may be disabled. */ }
}

async function boundedBody(response: Response): Promise<string> {
  const reader = response.body?.getReader();
  if (!reader) throw new Error('The local service returned an empty response.');
  const chunks: Uint8Array[] = [];
  let bytes = 0;
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      bytes += value.byteLength;
      if (bytes > 3 * 1_048_576) {
        await reader.cancel();
        throw new Error('The local service returned a response larger than the supported limit.');
      }
      chunks.push(value);
    }
  } finally { reader.releaseLock(); }
  const merged = new Uint8Array(bytes);
  let offset = 0;
  for (const chunk of chunks) { merged.set(chunk, offset); offset += chunk.byteLength; }
  return new TextDecoder().decode(merged);
}

async function performRequest<T>(path: string, body: unknown, includeErrorResponse: boolean): Promise<ApiResponse<T>> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), 45_000);
  const started = performance.now();
  try {
    const response = await fetch(path, {
      method: body === undefined ? 'GET' : 'POST',
      credentials: 'same-origin',
      cache: 'no-store',
      redirect: 'error',
      signal: controller.signal,
      headers: {
        ...(appCapability ? { 'X-App-Capability': appCapability } : {}),
        ...(body === undefined ? {} : { 'Content-Type': 'application/json', ...(csrfToken ? { 'X-CSRF-Token': csrfToken } : {}) }),
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    if (response.status === 204) return { body: undefined as T, rawBody: '', status: 204, durationMs: Math.round(performance.now() - started) };
    const raw = await boundedBody(response);
    let payload: unknown;
    try { payload = JSON.parse(raw); }
    catch { throw new Error('The local service returned an unreadable response. Relaunch the app and try again.'); }
    const details = { body: payload as T, rawBody: raw, status: response.status, durationMs: Math.round(performance.now() - started) };
    if (!response.ok) {
      const error = (payload as { error?: { code?: string; message?: string } })?.error;
      throw new ApiError(response.status, error?.code ?? 'request_failed', error?.message ?? 'The local request could not be completed.', includeErrorResponse ? details : undefined);
    }
    return details;
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new Error('The local service took too long to respond. Check that the app is still running.');
    }
    throw error;
  } finally { window.clearTimeout(timer); }
}

/** Normal application requests do not retain raw error bodies. */
export async function request<T>(path: string, body?: unknown): Promise<T> {
  return (await performRequest<T>(path, body, false)).body;
}

/** Developer inspection of an explicitly requested local response; never includes headers. */
export async function requestWithDetails<T>(path: string, body?: unknown): Promise<ApiResponse<T>> {
  return performRequest<T>(path, body, true);
}

export async function startSession(bootstrapToken: string | null): Promise<LocalSession> {
  if (bootstrapToken) clearSession();
  else {
    appCapability = savedCapability();
    if (!appCapability) throw new Error('Relaunch My Rivian Data and use the page it opens.');
  }
  try {
    const session = bootstrapToken
      ? await request<LocalSession>('/api/bootstrap', { token: bootstrapToken })
      : await request<LocalSession>('/api/session');
    if (!session.csrf_token || !['live', 'demo'].includes(session.mode) || (bootstrapToken && !session.app_capability)) {
      throw new Error('The local service did not establish a supported session.');
    }
    csrfToken = session.csrf_token;
    appCapability = session.app_capability ?? appCapability;
    if (appCapability) {
      try { window.sessionStorage?.setItem(CAPABILITY_KEY, appCapability); } catch { /* Reload requires relaunch when storage is disabled. */ }
    }
    return session;
  } catch (error) { clearSession(); throw error; }
}

export interface AccountStatus { state: 'signed_out' | 'mfa_required' | 'authenticated'; channel?: string }
export function readAccountStatus(value: unknown): AccountStatus {
  if (!value || typeof value !== 'object' || !('state' in value) ||
      !['signed_out', 'mfa_required', 'authenticated'].includes(String(value.state))) {
    throw new Error('The account connection returned an unexpected response. Please try again.');
  }
  return value as AccountStatus;
}

export function accountExpired(error: unknown): boolean {
  return error instanceof ApiError && ['account_auth_required', 'upstream_unauthorized', 'rivian_auth_required', 'not_authenticated'].includes(error.code);
}
