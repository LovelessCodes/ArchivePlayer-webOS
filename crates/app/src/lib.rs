//! Native webOS application scaffold.
//!
//! The UI/render/playback loop is intentionally not implemented yet. This crate
//! owns the startup state machine that the SDL event loop will drive.

use archiveplayer_tv_api::{ServerTarget, StoredSession};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupState {
    /// Reading the saved session and probing the server.
    Loading,
    /// No server configured yet.
    Setup,
    /// A server was selected but there is no valid token.
    Pairing { target: Option<ServerTarget> },
    /// A stored bearer token is available.
    Ready(StoredSession),
}

impl StartupState {
    /// The Android TV client's rule, kept identical here: saved token means
    /// Ready; a server without one means Pairing; nothing means Setup.
    pub fn from_session(target: Option<ServerTarget>, session: Option<StoredSession>) -> Self {
        match (target, session) {
            (_, Some(session)) => Self::Ready(session),
            (Some(target), None) => Self::Pairing {
                target: Some(target),
            },
            (None, None) => Self::Setup,
        }
    }

    pub fn needs_pairing(&self) -> bool {
        matches!(self, Self::Pairing { .. })
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    /// A 401 from any authenticated endpoint returns the app to pairing
    /// without discarding the remembered server.
    pub fn on_unauthorized(&self) -> Self {
        match self {
            Self::Ready(session) => Self::Pairing {
                target: Some(session.target.clone()),
            },
            other => other.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archiveplayer_tv_protocol::User;

    fn target() -> ServerTarget {
        ServerTarget::new("id", "Living room", "http://192.168.1.20:8080").unwrap()
    }

    fn session() -> StoredSession {
        StoredSession {
            target: target(),
            token: "0123456789abcdef".into(),
            user: User {
                id: 1,
                email: "owner@example.com".into(),
                display_name: None,
                created_at: None,
            },
            device_name: "webOS TV".into(),
        }
    }

    #[test]
    fn startup_follows_server_and_session() {
        assert_eq!(StartupState::from_session(None, None), StartupState::Setup);
        assert!(StartupState::from_session(Some(target()), None).needs_pairing());
        assert!(StartupState::from_session(Some(target()), Some(session())).is_ready());
    }

    #[test]
    fn unauthorized_returns_to_pairing_and_keeps_target() {
        let state = StartupState::Ready(session()).on_unauthorized();
        assert!(matches!(
            state,
            StartupState::Pairing {
                target: Some(ref target)
            } if target.id == "id"
        ));
    }
}
