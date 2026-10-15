export interface VaultStatus { backend: string; available: boolean; remembering: boolean; stored: boolean; error: string | null }
export function readVaultStatus(value: unknown): VaultStatus {
  const row = value as Partial<VaultStatus> | null;
  if (!row || typeof row.backend !== 'string' || !/^[a-zA-Z0-9_ -]{1,100}$/.test(row.backend) ||
      typeof row.available !== 'boolean' || typeof row.remembering !== 'boolean' || typeof row.stored !== 'boolean' ||
      !(row.error === null || typeof row.error === 'string' && /^[a-z_]{1,80}$/.test(row.error))) throw new Error('Remembered sign-in status could not be read. Sign-in for this session is still available.');
  return { backend: row.backend, available: row.available, remembering: row.remembering, stored: row.stored, error: row.error };
}
export function vaultMessage(status: VaultStatus): string {
  if (status.error) return status.remembering ? 'Your sign-in could not be saved in the protected credential store. This session can still be used.' : 'The protected credential store could not be updated. Try again when it is unlocked.';
  if (!status.available) return 'The protected credential store is unavailable. Sign-in will last for this app session.';
  return status.stored ? 'Your reusable sign-in is saved in the protected credential store. Disconnecting removes it.' : 'Your sign-in is kept in memory unless you choose to remember it on this device.';
}
