import { request, requestWithDetails } from './api.ts';
import type { ApiResponse } from './api.ts';
import type { AppMode } from './api.ts';
import { isValidationResult, parseVariables, validationsMatch } from './domain.ts';
import type { ValidationResult, WasmModule } from './domain.ts';

export interface ValidationPair { native: ValidationResult; wasm: ValidationResult; parity: boolean }

/** Both interfaces follow the same checks. The host independently revalidates execution. */
export async function validateOperation(
  wasm: WasmModule, operationId: string, variables: Record<string, unknown>, mode: AppMode = 'demo',
): Promise<ValidationPair> {
  const bounded = parseVariables(JSON.stringify(variables));
  const validator = mode === 'live' ? wasm.validate_live_request_json : wasm.validate_request_json;
  if (!validator) throw new Error('The data validator did not load. Relaunch the app.');
  const wasmResult: unknown = JSON.parse(validator(operationId, JSON.stringify(bounded)));
  if (!isValidationResult(wasmResult)) throw new Error('The local data check returned an unexpected result.');
  const native: unknown = await request('/api/validate', { operation_id: operationId, variables: bounded });
  if (!isValidationResult(native)) throw new Error('The app returned an unexpected validation result.');
  return { native, wasm: wasmResult, parity: validationsMatch(native, wasmResult) };
}

interface QueryEnvelope<T> {
  mode: string;
  data: { operation_id: string; source: string; synthetic: boolean; observed_at: string | null; received_at?: string; data: T };
}

export async function runQuery<T>(
  wasm: WasmModule, operationId: string, variables: Record<string, unknown>, mode: AppMode = 'demo',
): Promise<{ data: T; observedAt: string | null; receivedAt?: string }> {
  const pair = await validateOperation(wasm, operationId, variables, mode);
  if (!pair.parity || !pair.native.valid || !pair.wasm.valid) {
    throw new Error('The app could not check this request. Please try again.');
  }
  const response = await request<QueryEnvelope<T>>('/api/execute', { operation_id: operationId, variables });
  if (response?.mode !== mode || response.data?.synthetic !== (mode === 'demo') ||
      response.data.source !== (mode === 'demo' ? 'synthetic-demo' : 'rivian') || response.data.operation_id !== operationId ||
      !(typeof response.data.observed_at === 'string' && Number.isFinite(Date.parse(response.data.observed_at)) || mode === 'live' && response.data.observed_at === null) ||
      (mode === 'live' && (typeof response.data.received_at !== 'string' || !Number.isFinite(Date.parse(response.data.received_at))))) {
    throw new Error(mode === 'demo' ? 'The app returned data outside the expected sample format.' : 'The app returned data outside the expected Rivian response format.');
  }
  return { data: response.data.data, observedAt: response.data.observed_at, ...(mode === 'live' ? { receivedAt: response.data.received_at } : {}) };
}

/** Compatibility helper used by offline fixture tests. */
export const runDemoQuery = runQuery;

export interface PreparedCommand { confirmation_id: string; operation_id: string; variables: Record<string, unknown>; expiresAt: number }
const canonical = (value: unknown): string => JSON.stringify(value, (_key, item) => item && typeof item === 'object' && !Array.isArray(item) ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);
export function readPreparedCommand(value: unknown, operationId: string, variables: Record<string, unknown>, now = Date.now()): PreparedCommand {
  const row = value as { confirmation_id?: unknown; operation_id?: unknown; variables?: unknown; expires_in_seconds?: unknown } | null;
  if (!row || typeof row.confirmation_id !== 'string' || !/^[A-Za-z0-9_-]{16,256}$/.test(row.confirmation_id) || row.operation_id !== operationId || row.expires_in_seconds !== 120 ||
      canonical(row.variables) !== canonical(variables)) throw new Error('The app returned a different change than the one requested. Prepare the change again.');
  return { confirmation_id: row.confirmation_id, operation_id: operationId, variables: parseVariables(JSON.stringify(row.variables)), expiresAt: now + 120000 };
}
export async function prepareCommand(wasm: WasmModule, operationId: string, variables: Record<string, unknown>): Promise<PreparedCommand> {
  const pair = await validateOperation(wasm, operationId, variables, 'live');
  if (!pair.parity || !pair.native.valid || !pair.wasm.valid) throw new Error('Correct the validation issues before preparing this change.');
  return readPreparedCommand(await request('/api/commands/prepare', { operation_id: operationId, variables }), operationId, variables);
}
/** A confirmation is consumed once; callers never retry an ambiguous write. */
export async function executeCommand(prepared: PreparedCommand): Promise<ApiResponse<unknown>> {
  if (Date.now() >= prepared.expiresAt) throw new Error('This confirmation expired. Prepare the change again.');
  const result = await requestWithDetails<{ mode: string }>('/api/commands/execute', { confirmation_id: prepared.confirmation_id });
  if (result.body?.mode !== 'live') throw new Error('The command response did not match this live connection. Check your vehicle name before preparing another change.');
  return result;
}
