//! Device-authorization flow for the TV client.
//!
//! The server implements an RFC 8628-style grant: start a pairing session, show
//! the user code, then poll until an authenticated browser approves it. The
//! device code is a short-lived secret; the approval exchange returns the
//! long-lived bearer token stored by `SecretStore`.

use archiveplayer_tv_api::{connect_target, ApiClient, ApiError, ServerTarget, StoredSession};
use archiveplayer_tv_protocol::{DeviceStartRequest, DeviceStartResponse, DeviceTokenRequest};

pub const PLATFORM: &str = "webos";
pub const DEFAULT_CLIENT_NAME: &str = "ArchivePlayer TV";
pub const DEVICE_SCOPES: [&str; 2] = ["library:read", "progress:write"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingPoll {
    Pending,
    SlowDown { retry_after_seconds: u64 },
    Approved(Box<StoredSession>),
    Denied(String),
    Expired(String),
}

#[derive(Clone)]
pub struct PairingFlow {
    client: ApiClient,
    target: ServerTarget,
    device_name: String,
    start: DeviceStartResponse,
}

impl std::fmt::Debug for PairingFlow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingFlow")
            .field("target", &self.target)
            .field("device_name", &self.device_name)
            .field("user_code", &self.start.user_code)
            .field("device_code", &"<redacted>")
            .field("expires_in", &self.start.expires_in)
            .field("interval", &self.start.interval)
            .finish()
    }
}

impl PairingFlow {
    /// Probe the server, then create a pairing grant. The probe enforces the
    /// API-version guard and picks the LAN or remote address using the same
    /// policy as every other request.
    pub fn start(
        target: &ServerTarget,
        client_name: impl Into<String>,
        device_name: impl Into<String>,
    ) -> Result<Self, ApiError> {
        let connected = connect_target(target, None)?;
        let mut target = target.clone();
        target.server_version = Some(connected.health.version.clone());
        target.api_version = connected.health.api_version;
        let start = connected.client.device_start(&DeviceStartRequest {
            client_name: client_name.into(),
            platform: PLATFORM.into(),
            scopes: DEVICE_SCOPES
                .iter()
                .map(|scope| (*scope).to_string())
                .collect(),
        })?;
        Ok(Self {
            client: connected.client,
            target,
            device_name: device_name.into(),
            start,
        })
    }

    pub fn user_code(&self) -> &str {
        &self.start.user_code
    }

    pub fn verification_uri(&self) -> &str {
        &self.start.verification_uri
    }

    pub fn verification_uri_complete(&self) -> &str {
        &self.start.verification_uri_complete
    }

    pub fn expires_in_seconds(&self) -> i64 {
        self.start.expires_in
    }

    pub fn interval_seconds(&self) -> i64 {
        self.start.interval.max(1)
    }

    pub fn target(&self) -> &ServerTarget {
        &self.target
    }

    pub fn poll(&self) -> Result<PairingPoll, ApiError> {
        match self.client.device_token(&DeviceTokenRequest {
            device_code: self.start.device_code.clone(),
        }) {
            Ok(response) => Ok(PairingPoll::Approved(Box::new(StoredSession {
                target: self.target.clone(),
                token: response.token,
                user: response.user,
                device_name: self.device_name.clone(),
            }))),
            Err(error) => classify_poll_error(error),
        }
    }
}

/// Map the server's device-grant errors to UI states. Unknown errors remain
/// errors so the screen can show a real message instead of pretending to wait.
pub fn classify_poll_error(error: ApiError) -> Result<PairingPoll, ApiError> {
    match &error {
        ApiError::Status {
            code,
            message,
            retry_after,
            ..
        } => match code.as_deref() {
            Some("authorization_pending") => Ok(PairingPoll::Pending),
            Some("slow_down") => Ok(PairingPoll::SlowDown {
                retry_after_seconds: retry_after.unwrap_or(5).max(1),
            }),
            Some("access_denied") => Ok(PairingPoll::Denied(message.clone())),
            Some("expired_token") | Some("invalid_grant") => {
                Ok(PairingPoll::Expired(message.clone()))
            }
            _ => Err(error),
        },
        ApiError::Unauthorized => Ok(PairingPoll::Expired(
            "This pairing request is no longer valid.".into(),
        )),
        _ => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(code: &str, retry_after: Option<u64>) -> ApiError {
        ApiError::Status {
            status: if retry_after.is_some() { 429 } else { 400 },
            code: Some(code.into()),
            message: format!("{code} message"),
            retry_after,
        }
    }

    #[test]
    fn classifies_the_device_grant_states() {
        assert_eq!(
            classify_poll_error(status("authorization_pending", None)).unwrap(),
            PairingPoll::Pending
        );
        assert_eq!(
            classify_poll_error(status("slow_down", Some(9))).unwrap(),
            PairingPoll::SlowDown {
                retry_after_seconds: 9
            }
        );
        assert!(matches!(
            classify_poll_error(status("access_denied", None)).unwrap(),
            PairingPoll::Denied(message) if message.contains("access_denied")
        ));
        assert!(matches!(
            classify_poll_error(status("expired_token", None)).unwrap(),
            PairingPoll::Expired(_)
        ));
        assert!(classify_poll_error(status("server_bug", None)).is_err());
    }

    #[test]
    fn debug_redacts_the_device_code() {
        let flow = PairingFlow {
            client: ApiClient::new("http://192.168.1.20:8080").unwrap(),
            target: ServerTarget::new("id", "Living room", "http://192.168.1.20:8080").unwrap(),
            device_name: "webOS TV".into(),
            start: DeviceStartResponse {
                device_code: "0123456789abcdef".into(),
                user_code: "ABCD-EFGH".into(),
                verification_uri: "http://192.168.1.20:8080/#/activate".into(),
                verification_uri_complete: "http://192.168.1.20:8080/#/activate?code=ABCD-EFGH"
                    .into(),
                expires_in: 600,
                interval: 5,
            },
        };
        let text = format!("{flow:?}");
        assert!(!text.contains("0123456789abcdef"));
        assert!(text.contains("redacted"));
        assert!(text.contains("ABCD-EFGH"));
    }
}
