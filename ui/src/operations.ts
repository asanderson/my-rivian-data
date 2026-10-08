import { request } from './api.ts';
import { isValidationResult, parseVariables, validationsMatch } from './domain.ts';
import type { ValidationResult, WasmModule } from './domain.ts';

export interface ValidationPair { native: ValidationResult; wasm: ValidationResult; parity: boolean }

/** Both interfaces follow the same checks. The host independently revalidates execution. */
export async function validateOperation(
  wasm: WasmModule, operationId: string, variables: Record<string, unknown>,
): Promise<ValidationPair> {
  const bounded = parseVariables(JSON.stringify(variables));
  const wasmResult: unknown = JSON.parse(wasm.validate_request_json(operationId, JSON.stringify(bounded)));
  if (!isValidationResult(wasmResult)) throw new Error('The local data check returned an unexpected result.');
  const native: unknown = await request('/api/validate', { operation_id: operationId, variables: bounded });
  if (!isValidationResult(native)) throw new Error('The app returned an unexpected validation result.');
  return { native, wasm: wasmResult, parity: validationsMatch(native, wasmResult) };
}

interface DemoEnvelope<T> {
  mode: string;
  data: { operation_id: string; source: string; synthetic: boolean; observed_at: string; data: T };
}

export async function runDemoQuery<T>(
  wasm: WasmModule, operationId: string, variables: Record<string, unknown>,
): Promise<{ data: T; observedAt: string }> {
  const pair = await validateOperation(wasm, operationId, variables);
  if (!pair.parity || !pair.native.valid || !pair.wasm.valid) {
    throw new Error('The app could not check this sample data. Please try again.');
  }
  const response = await request<DemoEnvelope<T>>('/api/execute', { operation_id: operationId, variables });
  if (response?.mode !== 'demo' || response.data?.synthetic !== true ||
      response.data.source !== 'synthetic-demo' || response.data.operation_id !== operationId ||
      typeof response.data.observed_at !== 'string' || !Number.isFinite(Date.parse(response.data.observed_at))) {
    throw new Error('The app returned data outside the expected sample format.');
  }
  return { data: response.data.data, observedAt: response.data.observed_at };
}
