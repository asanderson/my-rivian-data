import { request } from './api.ts';
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
