use crate::ApiError;
use std::net::Ipv4Addr;
use url::Url;

/// Accept a bare host, host:port, or full http(s) URL and return a canonical
/// base URL. Public hosts must use HTTPS; plain HTTP is only for the LAN.
pub fn normalize_server_url(raw: &str) -> Result<Url, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::InvalidUrl("enter a server address".into()));
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme)
        .map_err(|error| ApiError::InvalidUrl(format!("that is not a valid address ({error})")))?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(ApiError::InvalidUrl(
            "only http:// and https:// addresses are supported".into(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ApiError::InvalidUrl(
            "server addresses cannot include a username or credential".into(),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(ApiError::InvalidUrl(
            "remove the query string and fragment from the server address".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| ApiError::InvalidUrl("the address has no host".into()))?
        .to_string();

    if url.scheme() == "http" && !is_private_host(&host) {
        return Err(ApiError::InvalidUrl(
            "public servers must use HTTPS; use an https:// address away from home".into(),
        ));
    }

    url.set_query(None);
    url.set_fragment(None);
    if url.path().is_empty() {
        url.set_path("/");
    }
    Ok(url)
}

/// Best-effort LAN detection for user-entered addresses. This is a convenience
/// guard, not a security boundary: HTTPS remains the only protection on a
/// hostile network.
pub fn is_private_host(host: &str) -> bool {
    let host = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".local") {
        return true;
    }
    if host == "::1" {
        return true;
    }
    if let Ok(address) = host.parse::<std::net::Ipv6Addr>() {
        // Unique-local (fc00::/7) and link-local (fe80::/10).
        let first = address.segments()[0];
        return (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80;
    }
    if let Ok(address) = host.parse::<Ipv4Addr>() {
        return address.is_loopback()
            || address.is_private()
            || address.is_link_local()
            || is_cgnat(address);
    }
    // A single-label host is a LAN name such as `livingroom` or `nas`.
    !host.contains('.')
}

fn is_cgnat(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

/// Human-facing form for a server tile: no scheme, no trailing slash.
pub fn display_server_url(raw: &str) -> String {
    raw.trim()
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lan_http_and_public_https() {
        assert!(normalize_server_url("192.168.1.20:8080").is_ok());
        assert!(normalize_server_url("http://nas.local:8080").is_ok());
        assert!(normalize_server_url("https://media.example.com").is_ok());
        assert!(normalize_server_url("https://media.example.com/app").is_ok());
    }

    #[test]
    fn rejects_public_http() {
        let error = normalize_server_url("http://media.example.com").unwrap_err();
        assert!(error.to_string().contains("HTTPS"));
    }

    #[test]
    fn rejects_credentials_and_queries() {
        assert!(normalize_server_url("https://user:password@example.com").is_err());
        assert!(normalize_server_url("https://example.com/?a=1").is_err());
        assert!(normalize_server_url("").is_err());
    }

    #[test]
    fn recognizes_private_ranges() {
        assert!(is_private_host("10.0.0.2"));
        assert!(is_private_host("172.20.1.1"));
        assert!(is_private_host("192.168.1.20"));
        assert!(is_private_host("100.101.102.103"));
        assert!(is_private_host("::1"));
        assert!(is_private_host("fd00::1"));
        assert!(!is_private_host("8.8.8.8"));
        assert!(!is_private_host("example.com"));
    }

    #[test]
    fn displays_without_scheme_or_slash() {
        assert_eq!(
            display_server_url("https://media.example.com/"),
            "media.example.com"
        );
        assert_eq!(
            display_server_url("http://192.168.1.20:8080/"),
            "192.168.1.20:8080"
        );
    }
}
