//! Blocking ArchivePlayer API client.
//!
//! The native webOS shell owns its UI thread and does not need an async
//! runtime. API calls are deliberately blocking; the app runs them on a worker
//! thread and posts the result back to the render loop.

use archiveplayer_tv_protocol as proto;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;
use std::time::Duration;
use thiserror::Error;
use url::Url;

mod server_url;
pub use server_url::{display_server_url, is_private_host, normalize_server_url};

/// The contract generation this client understands.
pub const API_VERSION: u32 = 1;
pub const DEFAULT_USER_AGENT: &str = concat!("ArchivePlayer-webOS/", env!("CARGO_PKG_VERSION"));
/// Artwork is small; a larger response is treated as an error instead of being
/// buffered on the television.
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("invalid server address: {0}")]
    InvalidUrl(String),
    #[error("could not reach the server: {0}")]
    Transport(String),
    #[error("the server returned HTTP {status}: {message}")]
    Status {
        status: u16,
        code: Option<String>,
        message: String,
        retry_after: Option<u64>,
    },
    #[error("this device is no longer paired")]
    Unauthorized,
    #[error("the server sent an unreadable response: {0}")]
    Decode(String),
    #[error("could not encode the request: {0}")]
    Encode(String),
    #[error("the server API is too new (server {server}, client {client})")]
    ApiVersion { server: u32, client: u32 },
}

impl ApiError {
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Unauthorized)
    }

    pub fn retry_after(&self) -> Option<u64> {
        match self {
            Self::Status { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Status { code, .. } => code.as_deref(),
            _ => None,
        }
    }
}

/// Download artwork from an absolute public URL.
///
/// The URL comes from the server's metadata (TMDB or the Internet Archive).
/// Private and loopback hosts are refused so a compromised or misconfigured
/// server cannot turn the TV into a scanner for the local network.
fn public_image_url(url: &str) -> Result<Url, ApiError> {
    let parsed = Url::parse(url).map_err(|error| ApiError::InvalidUrl(error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ApiError::InvalidUrl(
            "artwork must use HTTP or HTTPS".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| ApiError::InvalidUrl("artwork URL has no host".into()))?;
    if is_private_host(host) {
        return Err(ApiError::InvalidUrl(
            "refusing to fetch artwork from a private host".into(),
        ));
    }
    Ok(parsed)
}

pub fn fetch_image_bytes(url: &str) -> Result<Vec<u8>, ApiError> {
    public_image_url(url)?;
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(20))
        .timeout_write(Duration::from_secs(5))
        .user_agent(DEFAULT_USER_AGENT)
        .build();
    let response = agent
        .get(url)
        .set("Accept", "image/*")
        .call()
        .map_err(ureq_error)?;
    // A public URL can redirect to a private address; validate the final hop
    // before reading any body.
    public_image_url(response.get_url())?;
    if response
        .header("Content-Length")
        .and_then(|value| value.trim().parse::<usize>().ok())
        .is_some_and(|length| length > MAX_IMAGE_BYTES)
    {
        return Err(ApiError::Decode("artwork is too large".into()));
    }
    let mut reader = response.into_reader().take(MAX_IMAGE_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| ApiError::Decode(error.to_string()))?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ApiError::Decode("artwork is too large".into()));
    }
    Ok(bytes)
}

#[derive(Clone)]
pub struct ApiClient {
    agent: ureq::Agent,
    base_url: Url,
    token: Option<String>,
    client_header: String,
}

impl ApiClient {
    pub fn new(base_url: impl AsRef<str>) -> Result<Self, ApiError> {
        let base_url = normalize_server_url(base_url.as_ref())?;
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(6))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(15))
            .user_agent(DEFAULT_USER_AGENT)
            .build();
        Ok(Self {
            agent,
            base_url,
            token: None,
            client_header: format!("webos/{}", env!("CARGO_PKG_VERSION")),
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token;
        self
    }

    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
    }

    /// Health plus an API-version guard. Pair against this result rather than
    /// blindly posting a device request to an unknown server.
    pub fn probe(&self) -> Result<proto::Health, ApiError> {
        let health = self.health()?;
        if health.api_version > API_VERSION {
            return Err(ApiError::ApiVersion {
                server: health.api_version,
                client: API_VERSION,
            });
        }
        Ok(health)
    }

    pub fn health(&self) -> Result<proto::Health, ApiError> {
        self.get(&["api", "health"], &[], false)
    }

    pub fn auth_status(&self) -> Result<proto::AuthStatus, ApiError> {
        self.get(&["api", "auth", "status"], &[], true)
    }

    pub fn library(&self) -> Result<proto::LibraryResponse, ApiError> {
        self.get(&["api", "library"], &[], true)
    }

    pub fn continue_watching(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<proto::LibraryItem>, ApiError> {
        let limit = limit.map(|value| value.to_string());
        let query = limit
            .as_deref()
            .map(|limit| vec![("limit", limit)])
            .unwrap_or_default();
        self.get(&["api", "library", "continue-watching"], &query, true)
    }

    pub fn search(
        &self,
        query: &str,
        page: Option<u32>,
    ) -> Result<proto::ArchiveSearchResponse, ApiError> {
        let page = page.map(|value| value.max(1).to_string());
        let mut params = vec![("q", query)];
        if let Some(page) = page.as_deref() {
            params.push(("page", page));
        }
        self.get(&["api", "search"], &params, true)
    }

    /// Fetch details and a server-selected plan. `platform=webos` is what the
    /// native client should send; `file` selects one episode/quality.
    pub fn playback_details(
        &self,
        identifier: &str,
        platform: Option<&str>,
        file: Option<&str>,
    ) -> Result<proto::MediaDetails, ApiError> {
        let mut query = Vec::new();
        if let Some(platform) = platform {
            query.push(("platform", platform));
        }
        if let Some(file) = file {
            query.push(("file", file));
        }
        self.get(&["api", "playback", identifier], &query, true)
    }

    pub fn save_progress(
        &self,
        identifier: &str,
        request: &proto::ProgressRequest,
    ) -> Result<(), ApiError> {
        self.put_empty(&["api", "playback", identifier, "progress"], request)
    }

    pub fn mark_completed(
        &self,
        identifier: &str,
        request: &proto::CompletedRequest,
    ) -> Result<(), ApiError> {
        self.post_empty(&["api", "playback", identifier, "completed"], request)
    }

    pub fn device_start(
        &self,
        request: &proto::DeviceStartRequest,
    ) -> Result<proto::DeviceStartResponse, ApiError> {
        self.post(&["api", "device", "start"], request, false)
    }

    pub fn device_token(
        &self,
        request: &proto::DeviceTokenRequest,
    ) -> Result<proto::DeviceTokenResponse, ApiError> {
        self.post(&["api", "device", "token"], request, false)
    }

    /// Build a URL under the server root. Public for diagnostics and tests;
    /// path segments are percent-encoded by `url`.
    pub fn endpoint(&self, segments: &[&str]) -> Result<Url, ApiError> {
        let mut url = self.base_url.clone();
        {
            let mut path = url.path_segments_mut().map_err(|_| {
                ApiError::InvalidUrl("the server address cannot be used as a base URL".into())
            })?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    fn get<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        query: &[(&str, &str)],
        authenticated: bool,
    ) -> Result<T, ApiError> {
        let mut url = self.endpoint(segments)?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let response = self.execute(self.agent.get(url.as_str()), authenticated)?;
        decode(response)
    }

    fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        segments: &[&str],
        body: &T,
        authenticated: bool,
    ) -> Result<R, ApiError> {
        let url = self.endpoint(segments)?;
        let payload =
            serde_json::to_string(body).map_err(|error| ApiError::Encode(error.to_string()))?;
        let response = self
            .add_headers(
                self.agent
                    .post(url.as_str())
                    .set("Content-Type", "application/json; charset=utf-8"),
                authenticated,
            )
            .send_string(&payload)
            .map_err(ureq_error)?;
        decode(response)
    }

    fn put_empty<T: Serialize>(&self, segments: &[&str], body: &T) -> Result<(), ApiError> {
        let url = self.endpoint(segments)?;
        let payload =
            serde_json::to_string(body).map_err(|error| ApiError::Encode(error.to_string()))?;
        self.add_headers(
            self.agent
                .put(url.as_str())
                .set("Content-Type", "application/json; charset=utf-8"),
            true,
        )
        .send_string(&payload)
        .map_err(ureq_error)?;
        Ok(())
    }

    fn post_empty<T: Serialize>(&self, segments: &[&str], body: &T) -> Result<(), ApiError> {
        let url = self.endpoint(segments)?;
        let payload =
            serde_json::to_string(body).map_err(|error| ApiError::Encode(error.to_string()))?;
        self.add_headers(
            self.agent
                .post(url.as_str())
                .set("Content-Type", "application/json; charset=utf-8"),
            true,
        )
        .send_string(&payload)
        .map_err(ureq_error)?;
        Ok(())
    }

    fn execute(
        &self,
        request: ureq::Request,
        authenticated: bool,
    ) -> Result<ureq::Response, ApiError> {
        self.add_headers(request, authenticated)
            .call()
            .map_err(ureq_error)
    }

    fn add_headers(&self, request: ureq::Request, authenticated: bool) -> ureq::Request {
        let request = request
            .set("Accept", "application/json")
            .set("X-ArchivePlayer-Client", &self.client_header);
        if !authenticated {
            return request;
        }
        match self.token.as_deref() {
            Some(token) => request.set("Authorization", &format!("Bearer {token}")),
            None => request,
        }
    }
}

fn decode<T: DeserializeOwned>(response: ureq::Response) -> Result<T, ApiError> {
    let body = response
        .into_string()
        .map_err(|error| ApiError::Decode(error.to_string()))?;
    serde_json::from_str(&body).map_err(|error| ApiError::Decode(error.to_string()))
}

fn ureq_error(error: ureq::Error) -> ApiError {
    match error {
        ureq::Error::Status(status, response) => {
            let retry_after = response
                .header("Retry-After")
                .and_then(|value| value.trim().parse::<u64>().ok());
            let body = response.into_string().unwrap_or_default();
            let parsed: Option<proto::ApiErrorBody> = serde_json::from_str(&body).ok();
            let code = parsed.as_ref().and_then(|body| body.code.clone());
            let message = parsed
                .and_then(|body| body.message)
                .filter(|message| !message.trim().is_empty())
                .unwrap_or_else(|| format!("HTTP {status}"));
            if status == 401 {
                return ApiError::Unauthorized;
            }
            ApiError::Status {
                status,
                code,
                message,
                retry_after,
            }
        }
        ureq::Error::Transport(error) => ApiError::Transport(error.to_string()),
    }
}

/// A validated connection to one server. `used_remote` is true when the LAN
/// address failed at the transport level and the remote address answered.
pub struct ConnectedServer {
    pub client: ApiClient,
    pub health: proto::Health,
    pub used_remote: bool,
}

impl std::fmt::Debug for ConnectedServer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectedServer")
            .field("base_url", self.client.base_url())
            .field("health", &self.health)
            .field("used_remote", &self.used_remote)
            .field("token", &self.client.token().map(|_| "<redacted>"))
            .finish()
    }
}

/// Connect with the LAN-first policy. Only a transport failure falls back to
/// the remote URL; a 401, a 5xx, or a too-new API version is a real answer and
/// must not be hidden by trying another address.
pub fn connect_target(
    target: &ServerTarget,
    token: Option<String>,
) -> Result<ConnectedServer, ApiError> {
    let lan = target.lan_client()?.with_token(token.clone());
    match lan.probe() {
        Ok(health) => Ok(ConnectedServer {
            client: lan,
            health,
            used_remote: false,
        }),
        Err(ApiError::Transport(_)) => {
            let Some(remote) = target.remote_client()? else {
                return Err(ApiError::Transport(format!(
                    "could not reach {}",
                    display_server_url(&target.lan_url)
                )));
            };
            let remote = remote.with_token(token);
            let health = remote.probe()?;
            Ok(ConnectedServer {
                client: remote,
                health,
                used_remote: true,
            })
        }
        Err(error) => Err(error),
    }
}

/// A server address the user saved. LAN and remote URLs are separate so the
/// client can try the LAN first and fall back only on a transport failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTarget {
    pub id: String,
    pub name: String,
    pub lan_url: String,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub server_version: Option<String>,
    #[serde(default = "default_api_version")]
    pub api_version: u32,
}

fn default_api_version() -> u32 {
    1
}

impl ServerTarget {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        lan_url: impl AsRef<str>,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            id: id.into(),
            name: name.into(),
            lan_url: normalize_server_url(lan_url.as_ref())?.to_string(),
            remote_url: None,
            server_version: None,
            api_version: API_VERSION,
        })
    }

    pub fn with_remote(mut self, remote_url: impl AsRef<str>) -> Result<Self, ApiError> {
        self.remote_url = Some(normalize_server_url(remote_url.as_ref())?.to_string());
        Ok(self)
    }

    pub fn validated(&self) -> Result<Self, ApiError> {
        let mut target = self.clone();
        target.lan_url = normalize_server_url(&self.lan_url)?.to_string();
        target.remote_url = self
            .remote_url
            .as_deref()
            .map(normalize_server_url)
            .transpose()?
            .map(|url| url.to_string());
        Ok(target)
    }

    pub fn lan_client(&self) -> Result<ApiClient, ApiError> {
        ApiClient::new(&self.lan_url)
    }

    pub fn remote_client(&self) -> Result<Option<ApiClient>, ApiError> {
        self.remote_url.as_deref().map(ApiClient::new).transpose()
    }
}

/// What the platform store persists after pairing.
///
/// `Debug` is implemented by hand so a token cannot leak into a log line.
#[derive(Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSession {
    pub target: ServerTarget,
    pub token: String,
    pub user: proto::User,
    pub device_name: String,
}

impl std::fmt::Debug for StoredSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredSession")
            .field("target", &self.target)
            .field("token", &"<redacted>")
            .field("user", &self.user)
            .field("device_name", &self.device_name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_encoded_playback_urls() {
        let client = ApiClient::new("http://192.168.1.20:8080").unwrap();
        let url = client
            .endpoint(&["api", "playback", "movie with space"])
            .unwrap();
        assert_eq!(
            url.as_str(),
            "http://192.168.1.20:8080/api/playback/movie%20with%20space"
        );
    }

    #[test]
    fn keeps_a_reverse_proxy_path() {
        let client = ApiClient::new("https://media.example.com/archiveplayer").unwrap();
        assert_eq!(
            client.endpoint(&["api", "health"]).unwrap().as_str(),
            "https://media.example.com/archiveplayer/api/health"
        );
    }

    #[test]
    fn target_uses_lan_then_remote() {
        let target = ServerTarget::new("id", "Living room", "http://192.168.1.20:8080")
            .unwrap()
            .with_remote("https://media.example.com")
            .unwrap();
        assert_eq!(target.lan_url, "http://192.168.1.20:8080/");
        assert_eq!(
            target.remote_url.as_deref(),
            Some("https://media.example.com/")
        );
    }

    #[test]
    fn refuses_private_artwork_hosts() {
        for url in [
            "http://127.0.0.1:8080/poster.jpg",
            "http://192.168.1.2/poster.jpg",
            "http://localhost/poster.jpg",
            "file:///tmp/poster.jpg",
        ] {
            assert!(fetch_image_bytes(url).is_err(), "{url} should be refused");
        }
    }

    #[test]
    fn api_version_is_reported() {
        let error = ApiError::ApiVersion {
            server: 2,
            client: API_VERSION,
        };
        assert!(error.to_string().contains("server 2"));
    }
}
