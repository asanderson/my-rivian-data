//! Only OS credential stores may retain an explicitly opted-in owner session.
//! No filesystem or environment fallback exists. Passwords and MFA challenges
//! are never supplied to this module.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tokio::sync::watch;
use zeroize::Zeroizing;

use crate::ApiError;

const SERVICE: &str = "My Rivian Data";
const ACCOUNT: &str = "remembered-owner-session-v1";
const IO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VaultStatus {
    pub backend: &'static str,
    pub available: bool,
    pub remembering: bool,
    pub stored: bool,
    pub error: Option<&'static str>,
}

#[derive(Clone, Copy)]
pub(crate) enum StoreError {
    Unavailable,
}

pub(crate) trait SecretStore: Send + Sync {
    fn backend(&self) -> &'static str;
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError>;
    fn write(&self, secret: &[u8]) -> Result<(), StoreError>;
    fn delete(&self) -> Result<(), StoreError>;
}

// The lock contains no secret. Holding it for the native store lifetime prevents
// another running process from replacing or deleting the fixed credential entry.
struct NativeLease {
    file: File,
}
impl Drop for NativeLease {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}
impl NativeLease {
    fn acquire_at(path: &Path) -> Option<Self> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path).ok()?;
        fs2::FileExt::try_lock_exclusive(&file).ok()?;
        Some(Self { file })
    }
}

fn native_lock_path() -> Option<PathBuf> {
    let project = directories::ProjectDirs::from("org", "MyRivianData", "MyRivianData")?;
    let directory = project.data_local_dir();
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory).ok()?;
    Some(directory.join("owner-session-vault.lock"))
}

struct NativeStore {
    lease: Option<Arc<NativeLease>>,
}
impl NativeStore {
    fn new() -> Self {
        static LEASE: OnceLock<StdMutex<Weak<NativeLease>>> = OnceLock::new();
        let lease = LEASE.get_or_init(|| StdMutex::new(Weak::new()));
        let Ok(mut lease) = lease.lock() else {
            return Self { lease: None };
        };
        if let Some(existing) = lease.upgrade() {
            return Self {
                lease: Some(existing),
            };
        }
        let acquired = native_lock_path()
            .and_then(|path| NativeLease::acquire_at(&path))
            .map(Arc::new);
        if let Some(acquired) = &acquired {
            *lease = Arc::downgrade(acquired);
        }
        Self { lease: acquired }
    }
    fn available(&self) -> Result<(), StoreError> {
        if self.lease.is_some() {
            Ok(())
        } else {
            Err(StoreError::Unavailable)
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn native_entry() -> Result<keyring::Entry, StoreError> {
    // Explicit builders prevent keyring's default mock fallback from silently
    // accepting secrets when a native backend feature was accidentally omitted.
    #[cfg(target_os = "macos")]
    let builder = keyring::macos::default_credential_builder();
    #[cfg(target_os = "windows")]
    let builder = keyring::windows::default_credential_builder();
    #[cfg(target_os = "linux")]
    let builder = keyring::secret_service::default_credential_builder();
    builder
        .build(None, SERVICE, ACCOUNT)
        .map(keyring::Entry::new_with_credential)
        .map_err(|_| StoreError::Unavailable)
}

impl SecretStore for NativeStore {
    fn backend(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "macOS Keychain"
        } else if cfg!(target_os = "windows") {
            "Windows Credential Manager"
        } else if cfg!(target_os = "linux") {
            "Linux Secret Service"
        } else {
            "unavailable"
        }
    }

    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError> {
        self.available()?;
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        return match native_entry()?.get_secret() {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(StoreError::Unavailable),
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        Err(StoreError::Unavailable)
    }

    fn write(&self, secret: &[u8]) -> Result<(), StoreError> {
        self.available()?;
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        return native_entry()?
            .set_secret(secret)
            .map_err(|_| StoreError::Unavailable);
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = secret;
            Err(StoreError::Unavailable)
        }
    }

    fn delete(&self) -> Result<(), StoreError> {
        self.available()?;
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        return match native_entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(StoreError::Unavailable),
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        Err(StoreError::Unavailable)
    }
}

struct UnavailableStore;
impl SecretStore for UnavailableStore {
    fn backend(&self) -> &'static str {
        "unavailable"
    }
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError> {
        Err(StoreError::Unavailable)
    }
    fn write(&self, _: &[u8]) -> Result<(), StoreError> {
        Err(StoreError::Unavailable)
    }
    fn delete(&self) -> Result<(), StoreError> {
        Err(StoreError::Unavailable)
    }
}

#[derive(Default)]
struct Health {
    available: bool,
    stored: bool,
    error: Option<&'static str>,
}

pub(crate) struct Vault {
    store: Arc<dyn SecretStore>,
    gate: Arc<StdMutex<()>>,
    health: Arc<StdMutex<Health>>,
    io_timeout: Duration,
}

impl Vault {
    pub(crate) fn native() -> Self {
        static GATE: OnceLock<Arc<StdMutex<()>>> = OnceLock::new();
        Self {
            store: Arc::new(NativeStore::new()),
            gate: GATE.get_or_init(|| Arc::new(StdMutex::new(()))).clone(),
            health: Arc::new(StdMutex::new(Health::default())),
            io_timeout: IO_TIMEOUT,
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self::with_store(Arc::new(UnavailableStore))
    }

    pub(crate) fn with_store(store: Arc<dyn SecretStore>) -> Self {
        Self {
            store,
            gate: Arc::new(StdMutex::new(())),
            health: Arc::new(StdMutex::new(Health::default())),
            io_timeout: IO_TIMEOUT,
        }
    }

    pub(crate) fn status(&self, remembering: bool) -> VaultStatus {
        match self.health.lock() {
            Ok(health) => VaultStatus {
                backend: self.store.backend(),
                available: health.available,
                remembering,
                stored: health.stored,
                error: health.error,
            },
            Err(_) => VaultStatus {
                backend: self.store.backend(),
                available: false,
                remembering: false,
                stored: false,
                error: Some("vault_unavailable"),
            },
        }
    }

    fn mark_error(&self, code: &'static str) {
        if let Ok(mut health) = self.health.lock() {
            health.available = false;
            health.error = Some(code);
        }
    }

    pub(crate) async fn read(
        &self,
        generation: watch::Sender<u64>,
        epoch: u64,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, ApiError> {
        let store = self.store.clone();
        let gate = self.gate.clone();
        let health = self.health.clone();
        let operation = tokio::task::spawn_blocking(move || {
            let _guard = gate.lock().map_err(|_| unavailable())?;
            current(&generation, epoch)?;
            let result = store.read();
            current(&generation, epoch)?;
            let mut health = health.lock().map_err(|_| unavailable())?;
            match result {
                Ok(secret) => {
                    health.available = true;
                    health.stored = secret.is_some();
                    health.error = None;
                    current(&generation, epoch)?;
                    Ok(secret)
                }
                Err(_) => {
                    health.available = false;
                    health.error = Some("vault_unavailable");
                    Err(unavailable())
                }
            }
        });
        match tokio::time::timeout(self.io_timeout, operation).await {
            Ok(result) => result.map_err(|_| unavailable())?,
            Err(_) => {
                self.mark_error("vault_unavailable");
                Err(unavailable())
            }
        }
    }

    pub(crate) async fn write(
        &self,
        secret: Zeroizing<Vec<u8>>,
        generation: watch::Sender<u64>,
        epoch: u64,
    ) -> Result<(), ApiError> {
        let store = self.store.clone();
        let gate = self.gate.clone();
        let health = self.health.clone();
        let timeout_generation = generation.clone();
        let operation = tokio::task::spawn_blocking(move || {
            let _guard = gate.lock().map_err(|_| unavailable())?;
            current(&generation, epoch)?;
            let result = store.write(&secret);
            if result.is_ok()
                && let Err(error) = current(&generation, epoch)
            {
                // Native deletion may block too; never hold the status mutex
                // across OS I/O, so the asynchronous timeout remains bounded.
                let deleted = store.delete().is_ok();
                let mut health = health.lock().map_err(|_| unavailable())?;
                health.available = deleted;
                health.stored = !deleted;
                health.error = if deleted {
                    None
                } else {
                    Some("vault_delete_failed")
                };
                return Err(error);
            }
            let mut health = health.lock().map_err(|_| unavailable())?;
            match result {
                Ok(()) => {
                    health.available = true;
                    health.stored = true;
                    health.error = None;
                    Ok(())
                }
                Err(_) => {
                    health.available = false;
                    health.error = Some("vault_unavailable");
                    Err(unavailable())
                }
            }
        });
        match tokio::time::timeout(self.io_timeout, operation).await {
            Ok(result) => result.map_err(|_| unavailable())?,
            Err(_) => {
                // Blocking native calls cannot be forcibly interrupted. Epoch
                // invalidation makes the detached call refuse a late write or
                // delete it while still holding the serialized store gate.
                timeout_generation.send_if_modified(|generation| {
                    if *generation == epoch {
                        *generation = generation.wrapping_add(1);
                        true
                    } else {
                        false
                    }
                });
                self.mark_error("vault_unavailable");
                Err(unavailable())
            }
        }
    }

    pub(crate) async fn delete(&self) -> Result<(), ApiError> {
        let store = self.store.clone();
        let gate = self.gate.clone();
        let health = self.health.clone();
        let operation = tokio::task::spawn_blocking(move || {
            let _guard = gate.lock().map_err(|_| deletion_failed())?;
            let result = store.delete();
            let mut health = health.lock().map_err(|_| deletion_failed())?;
            match result {
                Ok(()) => {
                    health.available = true;
                    health.stored = false;
                    health.error = None;
                    Ok(())
                }
                Err(_) => {
                    health.available = false;
                    health.error = Some("vault_delete_failed");
                    Err(deletion_failed())
                }
            }
        });
        match tokio::time::timeout(self.io_timeout, operation).await {
            Ok(result) => result.map_err(|_| deletion_failed())?,
            Err(_) => {
                self.mark_error("vault_delete_failed");
                Err(deletion_failed())
            }
        }
    }
}

fn current(generation: &watch::Sender<u64>, epoch: u64) -> Result<(), ApiError> {
    if *generation.borrow() == epoch {
        Ok(())
    } else {
        Err(ApiError::new(
            "session_revoked",
            "This local session has ended. Reopen My Rivian Data.",
        ))
    }
}
fn unavailable() -> ApiError {
    ApiError::new(
        "vault_unavailable",
        "The operating-system vault is unavailable. You can sign in for this app session without remembering it.",
    )
}
fn deletion_failed() -> ApiError {
    ApiError::new(
        "vault_delete_failed",
        "Signed out locally, but the operating-system vault could not remove the remembered session. Unlock the vault and try signing out again.",
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn native_entry_has_one_exclusive_writer_and_lock_file_contains_no_secret() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let name = format!(
            "rivian-vault-lock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        );
        let directory = std::env::temp_dir().join(name);
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("owner-session-vault.lock");
        let first = NativeLease::acquire_at(&path).unwrap();
        assert!(NativeLease::acquire_at(&path).is_none());
        assert_eq!(std::fs::read(&path).unwrap(), b"");
        drop(first);
        let next = NativeLease::acquire_at(&path).unwrap();
        drop(next);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn process_without_exclusive_lease_never_accesses_native_entry() {
        let store = NativeStore { lease: None };
        assert!(store.read().is_err());
        assert!(store.write(b"test-secret").is_err());
        assert!(store.delete().is_err());
    }

    #[derive(Default)]
    pub(crate) struct WriteGate {
        pub(crate) entered: tokio::sync::Notify,
        released: StdMutex<bool>,
        ready: std::sync::Condvar,
    }
    impl WriteGate {
        pub(crate) fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.ready.notify_all();
        }
        fn wait(&self) {
            self.entered.notify_one();
            let guard = self.released.lock().unwrap();
            drop(self.ready.wait_while(guard, |released| !*released).unwrap());
        }
    }
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Default)]
    pub(crate) struct MemoryStore {
        secret: StdMutex<Option<Zeroizing<Vec<u8>>>>,
        pub(crate) unavailable: AtomicBool,
        pub(crate) delete_fails: AtomicBool,
        pub(crate) write_gate: StdMutex<Option<Arc<WriteGate>>>,
    }
    impl MemoryStore {
        pub(crate) fn read_for_test(&self) -> Option<Zeroizing<Vec<u8>>> {
            self.secret.lock().unwrap().clone()
        }
        pub(crate) fn replace_for_test(&self, bytes: Vec<u8>) {
            *self.secret.lock().unwrap() = Some(Zeroizing::new(bytes));
        }
    }
    impl SecretStore for MemoryStore {
        fn backend(&self) -> &'static str {
            "test memory"
        }
        fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError> {
            if self.unavailable.load(Ordering::SeqCst) {
                Err(StoreError::Unavailable)
            } else {
                Ok(self.read_for_test())
            }
        }
        fn write(&self, secret: &[u8]) -> Result<(), StoreError> {
            if let Some(gate) = self.write_gate.lock().unwrap().clone() {
                gate.wait();
            }
            if self.unavailable.load(Ordering::SeqCst) {
                Err(StoreError::Unavailable)
            } else {
                self.replace_for_test(secret.to_vec());
                Ok(())
            }
        }
        fn delete(&self) -> Result<(), StoreError> {
            if self.unavailable.load(Ordering::SeqCst) || self.delete_fails.load(Ordering::SeqCst) {
                Err(StoreError::Unavailable)
            } else {
                *self.secret.lock().unwrap() = None;
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn timed_out_native_write_is_deleted_when_the_blocking_store_returns() {
        let store = Arc::new(MemoryStore::default());
        let gate = Arc::new(WriteGate::default());
        *store.write_gate.lock().unwrap() = Some(gate.clone());
        let mut vault = Vault::with_store(store.clone());
        vault.io_timeout = Duration::from_millis(30);
        let (generation, _) = watch::channel(0);
        let write = vault.write(
            Zeroizing::new(b"test-secret".to_vec()),
            generation.clone(),
            0,
        );
        let result = write.await;
        assert_eq!(result.unwrap_err().code(), "vault_unavailable");
        assert_eq!(*generation.borrow(), 1);
        gate.release();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if !vault.status(false).stored && store.write_gate.try_lock().is_ok() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // Taking the serialized gate ensures the detached native call completed
        // its post-write stale-generation cleanup before checking the store.
        let operation_gate = vault.gate.clone();
        tokio::task::spawn_blocking(move || drop(operation_gate.lock().unwrap()))
            .await
            .unwrap();
        assert!(store.read_for_test().is_none());
    }
}
