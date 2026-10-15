import assert from 'node:assert/strict';
import test from 'node:test';
import { readVaultStatus } from '../src/account-data.ts';
import { readAccountStatus } from '../src/api.ts';

test('vault status accepts unavailable native storage without enabling persistence', () => {
  const status = { backend: 'linux_secret_service', available: false, remembering: false, stored: false, error: 'vault_unavailable' };
  assert.deepEqual(readVaultStatus(status), status);
});
test('vault status exposes only safe metadata and rejects malformed persistence state', () => {
  const status = { backend: 'linux_secret_service', available: true, remembering: true, stored: true, error: null };
  assert.deepEqual(readVaultStatus({ ...status, token: 'MUST_NOT_REACH_STATE' }), status);
  for (const value of [{ ...status, available: 'true' }, { ...status, error: 'secret payload' }, { ...status, remembering: null }]) assert.throws(() => readVaultStatus(value));
});
test('account outcomes retain safe persistence failures without retaining secret extras', () => {
  const vault = { backend: 'isolated_test_store', available: true, remembering: false, stored: false, error: 'vault_delete_failed' };
  assert.deepEqual(readAccountStatus({ state: 'signed_out', vault, remembered_session_removed: false, token: 'MUST_NOT_REACH_STATE' }), { state: 'signed_out', vault, remembered_session_removed: false });
  assert.throws(() => readAccountStatus({ state: 'authenticated', remembered_session_removed: 'false' }));
  assert.deepEqual(readAccountStatus({ state: 'signed_out', channel: null }), { state: 'signed_out' });
});
