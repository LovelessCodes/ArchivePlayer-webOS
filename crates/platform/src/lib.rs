//! Platform seams for the native shell.
//!
//! The webOS build will implement these traits with `luna-service`/file APIs;
//! host tests use the same interfaces with a temporary file or memory store.

use archiveplayer_tv_api::StoredSession;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Json(serde_json::Error),
    Unavailable,
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// One paired server. The native client has a single active session in v1;
/// keeping the trait plural makes a multi-server picker possible later.
pub trait SecretStore {
    fn load(&self) -> Result<Option<StoredSession>, StoreError>;
    fn save(&self, session: &StoredSession) -> Result<(), StoreError>;
    fn clear(&self) -> Result<(), StoreError>;
}

/// webOS packages have no system keychain. A private file in the app's data
/// directory is the realistic option; never put the token in `localStorage`
/// (that is browser storage) or a world-readable path.
pub struct FileSecretStore {
    path: PathBuf,
}

impl FileSecretStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SecretStore for FileSecretStore {
    fn load(&self) -> Result<Option<StoredSession>, StoreError> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError::Io(error)),
        }
    }

    fn save(&self, session: &StoredSession) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("tmp");
        let bytes = serde_json::to_vec(session)?;
        fs::write(&temporary, bytes)?;
        set_private_permissions(&temporary)?;
        fs::rename(&temporary, &self.path)?;
        set_private_permissions(&self.path)?;
        Ok(())
    }

    fn clear(&self) -> Result<(), StoreError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StoreError::Io(error)),
        }
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[derive(Default)]
pub struct MemorySecretStore {
    session: Mutex<Option<StoredSession>>,
}

impl SecretStore for MemorySecretStore {
    fn load(&self) -> Result<Option<StoredSession>, StoreError> {
        self.session
            .lock()
            .map(|session| session.clone())
            .map_err(|_| StoreError::Unavailable)
    }

    fn save(&self, session: &StoredSession) -> Result<(), StoreError> {
        *self.session.lock().map_err(|_| StoreError::Unavailable)? = Some(session.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), StoreError> {
        *self.session.lock().map_err(|_| StoreError::Unavailable)? = None;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredServer {
    pub id: String,
    pub name: String,
    pub lan_url: String,
    pub remote_url: Option<String>,
    pub api_version: u32,
    pub version: Option<String>,
}

pub trait ServerDiscovery {
    type Error: std::error::Error + Send + Sync + 'static;
    fn browse(&self, timeout: Duration) -> Result<Vec<DiscoveredServer>, Self::Error>;
}

/// Host/no-op discovery. The webOS implementation will browse
/// `_archiveplayer._tcp.local` and return the same shape.
pub struct NoopDiscovery;

impl ServerDiscovery for NoopDiscovery {
    type Error = std::convert::Infallible;

    fn browse(&self, _timeout: Duration) -> Result<Vec<DiscoveredServer>, Self::Error> {
        Ok(Vec::new())
    }
}

/// Display helper for a token without exposing it.
pub fn redact_token(token: &str) -> String {
    if token.len() <= 8 {
        "***".into()
    } else {
        format!("{}…{}", &token[..4], &token[token.len() - 4..])
    }
}

impl std::error::Error for StoreError {}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "invalid saved session: {error}"),
            Self::Unavailable => formatter.write_str("session store unavailable"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archiveplayer_tv_api::ServerTarget;
    use archiveplayer_tv_protocol::User;

    fn session() -> StoredSession {
        StoredSession {
            target: ServerTarget::new("id", "Living room", "http://192.168.1.20:8080").unwrap(),
            token: "0123456789abcdef".into(),
            user: User {
                id: 1,
                email: "owner@example.com".into(),
                display_name: Some("Owner".into()),
                created_at: None,
            },
            device_name: "webOS TV".into(),
        }
    }

    #[test]
    fn file_store_round_trips_and_clears() {
        let directory = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(directory.path().join("session.json"));
        assert_eq!(store.load().unwrap(), None);
        store.save(&session()).unwrap();
        assert_eq!(store.load().unwrap(), Some(session()));
        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn memory_store_works() {
        let store = MemorySecretStore::default();
        store.save(&session()).unwrap();
        assert!(store.load().unwrap().is_some());
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn debug_does_not_print_the_token() {
        let text = format!("{:?}", session());
        assert!(!text.contains("0123456789abcdef"));
        assert!(text.contains("redacted"));
    }
}
