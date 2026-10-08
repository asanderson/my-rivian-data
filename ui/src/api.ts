export class ApiError extends Error {
  status: number;
  code: string;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
  }
}

let csrfToken: string | undefined;
export function clearSession(): void { csrfToken = undefined; }

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
      if (bytes > 1_048_576) {
        await reader.cancel();
        throw new Error('The local service returned a response larger than the demo limit.');
      }
      chunks.push(value);
    }
  } finally { reader.releaseLock(); }
  const merged = new Uint8Array(bytes);
  let offset = 0;
  for (const chunk of chunks) { merged.set(chunk, offset); offset += chunk.byteLength; }
  return new TextDecoder().decode(merged);
}

export async function request<T>(path: string, body?: unknown): Promise<T> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), 12_000);
  try {
    const response = await fetch(path, {
      method: body === undefined ? 'GET' : 'POST',
      credentials: 'same-origin',
      cache: 'no-store',
      redirect: 'error',
      signal: controller.signal,
      headers: body === undefined ? {} : {
        'Content-Type': 'application/json',
        ...(csrfToken ? { 'X-CSRF-Token': csrfToken } : {}),
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    if (response.status === 204) return undefined as T;
    const raw = await boundedBody(response);
    let payload: unknown;
    try { payload = JSON.parse(raw); }
    catch { throw new Error('The local service returned an unreadable response. Relaunch the app and try again.'); }
    if (!response.ok) {
      const error = (payload as { error?: { code?: string; message?: string } })?.error;
      throw new ApiError(response.status, error?.code ?? 'request_failed', error?.message ?? 'The local request could not be completed.');
    }
    return payload as T;
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new Error('The local service took too long to respond. Check that the app is still running.');
    }
    throw error;
  } finally { window.clearTimeout(timer); }
}

export async function startSession(bootstrapToken: string | null): Promise<void> {
  const session = bootstrapToken
    ? await request<{ csrf_token: string; mode: string }>('/api/bootstrap', { token: bootstrapToken })
    : await request<{ csrf_token: string; mode: string }>('/api/session');
  if (!session.csrf_token || session.mode !== 'demo') throw new Error('The local service did not establish an offline demo session.');
  csrfToken = session.csrf_token;
}
