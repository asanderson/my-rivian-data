export interface Operation {
  id: string;
  label: string;
  family: string;
  kind: 'query' | 'mutation' | 'subscription';
  description: string;
  status: 'demo' | 'live' | 'planned' | 'blocked';
  upstream_operation?: string;
  upstream_document?: string;
  endpoint?: string;
  requires_vehicle: boolean;
  example_variables: Record<string, unknown>;
  source_url: string;
}

export interface Vehicle {
  id: string;
  name: string;
  model: string | null;
  model_year: number | null;
  battery_percent: number | null;
  estimated_range_km: number | null;
  odometer_km: number | null;
  locked: boolean | null;
  charging_state: string | null;
  temperature_celsius: number | null;
  location: { latitude: number; longitude: number; accuracy_m: number | null } | null;
  observed_at: string | null;
  source: string;
}

export interface ValidationIssue { field: string; message: string }
export interface ValidationResult { valid: boolean; issues: ValidationIssue[] }
export interface WasmModule {
  default: () => Promise<unknown>;
  validate_request_json: (operationId: string, variables: string) => string;
  validate_live_request_json?: (operationId: string, variables: string) => string;
}

export const MAX_VARIABLE_BYTES = 16 * 1024;

/** Bounds the same input before either native or WebAssembly processing. */
export function parseVariables(text: string): Record<string, unknown> {
  if (new TextEncoder().encode(text).byteLength > MAX_VARIABLE_BYTES) {
    throw new Error('Variables exceed the 16 KiB limit. Shorten the request and try again.');
  }
  let value: unknown;
  try { value = JSON.parse(text); }
  catch { throw new Error('Enter valid JSON. Check quotation marks, commas, and brackets.'); }
  if (value === null || Array.isArray(value) || typeof value !== 'object') {
    throw new Error('Variables must be a JSON object, such as {"vehicle_id":"demo-r1t-001"}.');
  }
  return value as Record<string, unknown>;
}

export function isValidationResult(value: unknown): value is ValidationResult {
  if (!value || typeof value !== 'object') return false;
  const result = value as Partial<ValidationResult>;
  return typeof result.valid === 'boolean' && Array.isArray(result.issues) &&
    result.issues.every(issue => !!issue && typeof issue.field === 'string' && typeof issue.message === 'string');
}

/** Issue ordering is not part of the validator contract. */
export function validationsMatch(a: ValidationResult, b: ValidationResult): boolean {
  const normalize = (r: ValidationResult) => JSON.stringify({
    valid: r.valid,
    issues: r.issues.map(issue => [issue.field, issue.message]).sort((x, y) => JSON.stringify(x).localeCompare(JSON.stringify(y))),
  });
  return normalize(a) === normalize(b);
}

export function safeSourceUrl(value: string): string | undefined {
  try {
    const url = new URL(value);
    return url.protocol === 'https:' ? url.href : undefined;
  } catch { return undefined; }
}
