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
        let mut data = Vec::new();
        let mut buffer = [0_u8; 4096];
        let header_end = loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break data.len();
            }
            data.extend_from_slice(&buffer[..read]);
            if let Some(position) = data.windows(4).position(|window| window == b"\r\n\r\n") {
                break position + 4;
            }
        };
        let request_headers = String::from_utf8_lossy(&data[..header_end]).to_ascii_lowercase();
        let content_length = request_headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        while data.len() < header_end + content_length {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..read]);
        }
        let request = String::from_utf8_lossy(&data).to_string();
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

#[test]
fn adds_to_the_library_with_a_post() {
    let (base, server) = serve_once(
        "200 OK",
        r#"{"id":7,"identifier":"night_of_the_living_dead","title":"Night of the Living Dead"}"#,
        "",
    );
    let client = ApiClient::new(&base)
        .unwrap()
        .with_token(Some("token".into()));
    let item = client
        .add_to_library("night_of_the_living_dead", Some("Horror"))
        .unwrap();
    assert_eq!(item.id, 7);
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /api/library HTTP/1.1"));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer token"));
    assert!(request.contains("\"identifier\":\"night_of_the_living_dead\""));
    assert!(request.contains("\"categoryName\":\"Horror\""));
}

#[test]
fn searches_the_archive_with_a_url_encoded_query() {
    let (base, server) = serve_once(
        "200 OK",
        r#"{"results":[],"hasMoreResults":false,"currentPage":1,"totalResults":0}"#,
        "",
    );
    let client = ApiClient::new(&base)
        .unwrap()
        .with_token(Some("token".into()));
    let response = client.search("night of the living dead", Some(2)).unwrap();
    assert_eq!(response.current_page, 1);
    let request = server.join().unwrap();
    assert!(request.starts_with("GET /api/search?"));
    assert!(request.contains("q=night+of+the+living+dead"));
    assert!(request.contains("page=2"));
}

#[test]
fn sends_progress_as_a_put_with_camel_case_fields() {
    let (base, server) = serve_once("204 No Content", "", "");
    let client = ApiClient::new(&base)
        .unwrap()
        .with_token(Some("token".into()));
    client
        .save_progress(
            "movie id",
            &archiveplayer_tv_protocol::ProgressRequest {
                file_name: "movie.mp4".into(),
                position_seconds: 42,
                duration_seconds: 600,
            },
        )
        .unwrap();
    let request = server.join().unwrap();
    assert!(request.starts_with("PUT /api/playback/movie%20id/progress HTTP/1.1"));
    assert!(request.contains("\"fileName\":\"movie.mp4\""));
    assert!(request.contains("\"positionSeconds\":42"));
    assert!(request.contains("\"durationSeconds\":600"));
}

#[test]
fn marks_completion_with_the_item_flag() {
    let (base, server) = serve_once("204 No Content", "", "");
    let client = ApiClient::new(&base)
        .unwrap()
        .with_token(Some("token".into()));
    client
        .mark_completed(
            "movie",
            &archiveplayer_tv_protocol::CompletedRequest {
                file_name: "movie.mp4".into(),
                item_completed: true,
            },
        )
        .unwrap();
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /api/playback/movie/completed HTTP/1.1"));
    assert!(request.contains("\"itemCompleted\":true"));
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
