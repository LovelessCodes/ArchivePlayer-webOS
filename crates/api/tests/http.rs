use archiveplayer_tv_api::{ApiClient, ApiError};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

fn serve_once(status: &str, body: &str, headers: &str) -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let status = status.to_string();
    let body = body.to_string();
    let headers = headers.to_string();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = vec![0_u8; 16 * 1024];
        let length = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..length]).to_string();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
        request
    });
    (format!("http://{address}"), handle)
}

#[test]
fn probes_health_and_reads_the_version() {
    let (base, server) = serve_once(
        "200 OK",
        r#"{"status":"ok","version":"0.5.0","apiVersion":1}"#,
        "",
    );
    let client = ApiClient::new(&base).unwrap();
    let health = client.probe().unwrap();
    assert_eq!(health.version, "0.5.0");
    assert_eq!(health.api_version, 1);
    let request = server.join().unwrap();
    assert!(request.starts_with("GET /api/health HTTP/1.1"));
}

#[test]
fn sends_the_bearer_token() {
    let (base, server) = serve_once(
        "200 OK",
        r#"{"categories":[],"categoryNames":[],"totalItems":0}"#,
        "",
    );
    let client = ApiClient::new(&base)
        .unwrap()
        .with_token(Some("abc123".into()));
    let library = client.library().unwrap();
    assert_eq!(library.total_items, 0);
    let request = server.join().unwrap().to_ascii_lowercase();
    assert!(request.contains("authorization: bearer abc123"));
}

#[test]
fn maps_401_to_unauthorized() {
    let (base, server) = serve_once(
        "401 Unauthorized",
        r#"{"code":"unauthorized","message":"expired"}"#,
        "",
    );
    let client = ApiClient::new(&base)
        .unwrap()
        .with_token(Some("old".into()));
    let error = client.library().unwrap_err();
    assert!(error.is_unauthorized());
    server.join().unwrap();
}

#[test]
fn exposes_retry_after() {
    let (base, server) = serve_once(
        "429 Too Many Requests",
        r#"{"code":"slow_down","message":"wait"}"#,
        "Retry-After: 7\r\n",
    );
    let client = ApiClient::new(&base).unwrap();
    let error = client
        .device_token(&archiveplayer_tv_protocol::DeviceTokenRequest {
            device_code: "device".into(),
        })
        .unwrap_err();
    assert_eq!(error.code(), Some("slow_down"));
    assert_eq!(error.retry_after(), Some(7));
    match error {
        ApiError::Status { status, .. } => assert_eq!(status, 429),
        other => panic!("unexpected error: {other:?}"),
    }
    server.join().unwrap();
}

#[test]
fn encodes_playback_platform_and_file() {
    let (base, server) = serve_once(
        "200 OK",
        r#"{"identifier":"movie","title":"Movie","playbackPlan":null}"#,
        "",
    );
    let client = ApiClient::new(&base)
        .unwrap()
        .with_token(Some("token".into()));
    let details = client
        .playback_details("movie id", Some("webos"), Some("episode one.mp4"))
        .unwrap();
    assert_eq!(details.identifier, "movie");
    let request = server.join().unwrap();
    assert!(request.starts_with("GET /api/playback/movie%20id?"));
    assert!(request.contains("platform=webos"));
    assert!(request.contains("file=episode+one.mp4"));
}

fn closed_local_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{address}")
}

#[test]
fn lan_http_error_does_not_fall_back_to_remote() {
    let (lan, server) = serve_once("500 Internal Server Error", "{}", "");
    let target = archiveplayer_tv_api::ServerTarget::new("id", "Living room", &lan)
        .unwrap()
        .with_remote(closed_local_url())
        .unwrap();
    let error = archiveplayer_tv_api::connect_target(&target, None).unwrap_err();
    match error {
        ApiError::Status { status, .. } => assert_eq!(status, 500),
        other => panic!("unexpected error: {other:?}"),
    }
    server.join().unwrap();
}

#[test]
fn transport_failure_falls_back_to_remote() {
    let (remote, server) = serve_once(
        "200 OK",
        r#"{"status":"ok","version":"0.5.0","apiVersion":1}"#,
        "",
    );
    let target = archiveplayer_tv_api::ServerTarget::new("id", "Living room", closed_local_url())
        .unwrap()
        .with_remote(&remote)
        .unwrap();
    let connected = archiveplayer_tv_api::connect_target(&target, Some("token".into())).unwrap();
    assert!(connected.used_remote);
    assert_eq!(connected.health.version, "0.5.0");
    assert_eq!(connected.client.token(), Some("token"));
    server.join().unwrap();
}

#[test]
fn newer_api_version_is_a_terminal_error() {
    let (lan, server) = serve_once(
        "200 OK",
        r#"{"status":"ok","version":"9.0.0","apiVersion":2}"#,
        "",
    );
    let target = archiveplayer_tv_api::ServerTarget::new("id", "Living room", &lan)
        .unwrap()
        .with_remote(closed_local_url())
        .unwrap();
    let error = archiveplayer_tv_api::connect_target(&target, None).unwrap_err();
    assert!(matches!(
        error,
        ApiError::ApiVersion {
            server: 2,
            client: 1
        }
    ));
    server.join().unwrap();
}
