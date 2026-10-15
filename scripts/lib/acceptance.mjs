import { run } from './process.mjs';
import { createHash } from 'node:crypto';

export const acceptanceModels = ['R1T', 'R1S', 'R2', 'R3', 'R3X'];
export const acceptanceStatuses = ['passed', 'failed', 'blocked', 'not-tested'];

function exactFields(value, expected) {
  if (!value || typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).sort().join(',') !== [...expected].sort().join(',')) {
    throw new Error('Acceptance fields must match the sanitized record schema.');
  }
}

export function sanitizeAcceptance(record, allowedIds) {
  exactFields(record, ['approved', 'model', 'firmware', 'operations']);
  if (record.approved !== true) throw new Error('Acceptance must be explicitly owner approved.');
  if (!acceptanceModels.includes(record.model)) throw new Error('Use an allowlisted vehicle model only.');
  if (typeof record.firmware !== 'string' || !/^\d{4}(?:\.\d{1,6}){1,3}$/u.test(record.firmware)) {
    throw new Error('Acceptance firmware must contain only a numeric dotted firmware version.');
  }
  if (!(allowedIds instanceof Set) || !Array.isArray(record.operations) || record.operations.length > allowedIds.size) {
    throw new Error('Acceptance operations must use the catalog allowlist.');
  }
  const seen = new Set();
  const operations = record.operations.map(operation => {
    exactFields(operation, ['id', 'status']);
    if (typeof operation.id !== 'string' || !allowedIds.has(operation.id) || seen.has(operation.id)
      || !acceptanceStatuses.includes(operation.status)) throw new Error('Invalid acceptance operation or status.');
    seen.add(operation.id);
    return { id: operation.id, status: operation.status };
  });
  return { approved: true, model: record.model, firmware: record.firmware, operations };
}

// Export from the selected executable so recorded IDs match the exact build
// under test. This CLI operation exits without a local session or any network.
export function acceptanceCatalogEvidence(binary) {
  let exportValue;
  try { exportValue = JSON.parse(run(binary, ['--catalog-json'], { capture: true })); }
  catch { throw new Error('The selected executable must support the credential-free --catalog-json export.'); }
  const ids = exportValue.catalog?.map(entry => entry.id);
  if (!Array.isArray(ids) || !ids.length || ids.length > 1000 || ids.some(id => typeof id !== 'string' || !/^[a-z][a-z0-9-]{0,100}$/u.test(id)) || new Set(ids).size !== ids.length) throw new Error('Cannot read the local operation catalog.');
  return { ids: new Set(ids), sha256: createHash('sha256').update(JSON.stringify(exportValue.catalog)).digest('hex') };
}
export const acceptanceCatalog = binary => acceptanceCatalogEvidence(binary).ids;
