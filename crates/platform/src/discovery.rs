//! mDNS discovery for ArchivePlayer servers on the local network.
//!
//! The TV never talks to Archive.org directly, and the server's address can
//! change between DHCP leases. Browsing `_archiveplayer._tcp.local` gives the
//! client the current address plus the server's stable ID, which lets a paired
//! TV reconnect without asking the user to type an IP address.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use archiveplayer_tv_api::normalize_server_url;
use mdns_sd::{ScopedIp, ServiceDaemon, ServiceEvent, TxtProperties};

use super::{DiscoveredServer, ServerDiscovery};

pub const SERVICE_TYPE: &str = "_archiveplayer._tcp.local.";

#[derive(Debug)]
pub enum DiscoveryError {
    Mdns(mdns_sd::Error),
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mdns(error) => write!(formatter, "mDNS discovery failed: {error}"),
        }
    }
}

impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mdns(error) => Some(error),
        }
    }
}

impl From<mdns_sd::Error> for DiscoveryError {
    fn from(error: mdns_sd::Error) -> Self {
        Self::Mdns(error)
    }
}

/// Browse for servers advertised by `archiveplayer-server`.
///
/// Browsing waits for the full timeout so one slow responder cannot hide the
/// other servers on the LAN. Results are deduplicated by the stable server ID
/// and sorted by name, making the first item deterministic when the UI has no
/// saved choice.
#[derive(Debug, Default, Clone, Copy)]
pub struct MdnsDiscovery;

impl ServerDiscovery for MdnsDiscovery {
    type Error = DiscoveryError;

    fn browse(&self, timeout: Duration) -> Result<Vec<DiscoveredServer>, Self::Error> {
        let timeout = timeout.max(Duration::from_millis(100));
        let daemon = ServiceDaemon::new()?;
        let receiver = daemon.browse(SERVICE_TYPE)?;
        let deadline = Instant::now() + timeout;
        let mut found = BTreeMap::<String, DiscoveredServer>::new();

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match receiver.recv_timeout(remaining) {
                Ok(ServiceEvent::ServiceResolved(service)) => {
                    if let Some(server) = parse_service(
                        service.get_fullname(),
                        service.get_port(),
                        service.get_addresses(),
                        service.get_properties(),
                    ) {
                        insert_server(&mut found, server);
                    }
                }
                Ok(_) => {}
                Err(mdns_sd::RecvTimeoutError::Timeout) => break,
                // The daemon stopped before the timeout. Keep whatever resolved
                // so far rather than turning a transient shutdown into an empty
                // error screen.
                Err(mdns_sd::RecvTimeoutError::Disconnected) => break,
            }
        }

        let _ = daemon.stop_browse(SERVICE_TYPE);
        let _ = daemon.shutdown();

        let mut servers: Vec<_> = found.into_values().collect();
        servers.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(servers)
    }
}

fn parse_service(
    fullname: &str,
    port: u16,
    addresses: &HashSet<ScopedIp>,
    properties: &TxtProperties,
) -> Option<DiscoveredServer> {
    let id = txt(properties, "id")?;
    // The server writes 64 hex characters. Require a stable-looking ID here so
    // a short placeholder can never be used to match a saved session.
    if !(12..=128).contains(&id.len()) {
        return None;
    }
    let api_version = txt(properties, "api")?.parse::<u32>().ok()?;
    // `addr` is the server's own routing hint. It disambiguates hosts with
    // multiple interfaces, where DNS-SD can return a Docker bridge or a
    // link-local IPv6 address before the real LAN address.
    let address = txt(properties, "addr")
        .and_then(|value| value.parse::<IpAddr>().ok())
        .filter(|address| usable_address(*address))
        .or_else(|| best_address(addresses))?;
    let instance = instance_name(fullname)?;
    let name = txt(properties, "name").unwrap_or(&instance);
    let path = txt(properties, "path").unwrap_or("/");
    let host = match address {
        IpAddr::V4(address) => address.to_string(),
        IpAddr::V6(address) => format!("[{address}]"),
    };
    let suffix = normalized_path(path);
    let lan_url = normalize_server_url(&format!("http://{host}:{port}{suffix}"))
        .ok()?
        .to_string();
    let remote_url = txt(properties, "url")
        .and_then(|url| normalize_server_url(url).ok())
        .map(|url| url.to_string())
        .filter(|url| url != &lan_url);

    Some(DiscoveredServer {
        id: id.to_string(),
        name: name.to_string(),
        lan_url,
        remote_url,
        api_version,
        version: txt(properties, "version").map(str::to_string),
    })
}

fn txt<'a>(properties: &'a TxtProperties, key: &str) -> Option<&'a str> {
    properties
        .get_property_val_str(key)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn instance_name(fullname: &str) -> Option<String> {
    let suffix = format!(".{SERVICE_TYPE}");
    let raw = fullname
        .strip_suffix(&suffix)
        .or_else(|| fullname.split("._").next())?;
    let name = raw.trim().trim_end_matches('.');
    (!name.is_empty()).then(|| name.to_string())
}

fn normalized_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() || path == "/" {
        String::new()
    } else {
        format!("/{}", path.trim_matches('/'))
    }
}

fn best_address(addresses: &HashSet<ScopedIp>) -> Option<IpAddr> {
    addresses
        .iter()
        .map(ScopedIp::to_ip_addr)
        .filter(|address| usable_address(*address))
        .min_by_key(|address| (address_rank(*address), *address))
}

fn usable_address(address: IpAddr) -> bool {
    !address.is_unspecified()
        && !address.is_loopback()
        && !address.is_multicast()
        && !matches!(address, IpAddr::V6(address) if address.is_unicast_link_local())
}

fn address_rank(address: IpAddr) -> u8 {
    match address {
        IpAddr::V4(address) if address.is_private() && !address.is_link_local() => 0,
        IpAddr::V4(address) if address.is_link_local() => 4,
        IpAddr::V4(_) => 1,
        IpAddr::V6(address) if (address.segments()[0] & 0xfe00) == 0xfc00 => 2,
        IpAddr::V6(address) if address.is_unicast_link_local() => 5,
        IpAddr::V6(_) => 3,
    }
}

fn insert_server(found: &mut BTreeMap<String, DiscoveredServer>, server: DiscoveredServer) {
    match found.get_mut(&server.id) {
        None => {
            found.insert(server.id.clone(), server);
        }
        Some(existing) => {
            let old_rank = lan_rank(&existing.lan_url);
            let new_rank = lan_rank(&server.lan_url);
            if new_rank < old_rank || (new_rank == old_rank && server.lan_url < existing.lan_url) {
                existing.lan_url = server.lan_url;
                existing.name = server.name;
            }
            if existing.remote_url.is_none() {
                existing.remote_url = server.remote_url;
            }
            if existing.version.is_none() {
                existing.version = server.version;
            }
        }
    }
}

fn lan_rank(url: &str) -> u8 {
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let host = if let Some(rest) = rest.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else {
        rest.split(':').next().unwrap_or(rest)
    };
    host.parse::<IpAddr>().map(address_rank).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdns_sd::ServiceInfo;

    fn advertisement(name: &str, properties: &[(&str, &str)]) -> ServiceInfo {
        ServiceInfo::new(
            SERVICE_TYPE,
            name,
            "archiveplayer-test.local.",
            "127.0.0.1,192.168.1.44",
            8080,
            properties,
        )
        .unwrap()
    }

    fn parse(info: &ServiceInfo) -> Option<DiscoveredServer> {
        let addresses: HashSet<ScopedIp> = info
            .get_addresses()
            .iter()
            .copied()
            .map(ScopedIp::from)
            .collect();
        parse_service(
            info.get_fullname(),
            info.get_port(),
            &addresses,
            info.get_properties(),
        )
    }

    #[test]
    fn reads_the_server_contract_and_prefers_lan_ipv4() {
        let info = advertisement(
            "Living Room",
            &[
                ("name", "Living Room NAS"),
                ("id", &"a".repeat(64)),
                ("api", "1"),
                ("version", "0.5.0"),
                ("path", "/"),
                ("url", "https://media.example.com"),
            ],
        );
        let server = parse(&info).unwrap();
        assert_eq!(server.id, "a".repeat(64));
        assert_eq!(server.name, "Living Room NAS");
        assert_eq!(server.lan_url, "http://192.168.1.44:8080/");
        assert_eq!(
            server.remote_url.as_deref(),
            Some("https://media.example.com/")
        );
        assert_eq!(server.api_version, 1);
        assert_eq!(server.version.as_deref(), Some("0.5.0"));
    }

    #[test]
    fn the_txt_address_hint_beats_ambiguous_resolved_addresses() {
        let id = "f".repeat(64);
        let info = advertisement(
            "Living Room",
            &[("id", &id), ("api", "1"), ("addr", "10.0.0.7")],
        );
        let server = parse(&info).unwrap();
        assert_eq!(server.lan_url, "http://10.0.0.7:8080/");
    }

    #[test]
    fn an_invalid_address_hint_falls_back_to_the_resolved_address() {
        let id = "0".repeat(64);
        let info = advertisement(
            "Living Room",
            &[("id", &id), ("api", "1"), ("addr", "not-an-ip")],
        );
        let server = parse(&info).unwrap();
        assert_eq!(server.lan_url, "http://192.168.1.44:8080/");
    }

    #[test]
    fn falls_back_to_the_instance_name_and_keeps_a_subpath() {
        let id = "b".repeat(64);
        let info = advertisement(
            "Attic",
            &[("id", &id), ("api", "2"), ("path", "/archiveplayer/")],
        );
        let server = parse(&info).unwrap();
        assert_eq!(server.name, "Attic");
        assert_eq!(server.lan_url, "http://192.168.1.44:8080/archiveplayer");
        assert_eq!(server.api_version, 2);
        assert!(server.version.is_none());
    }

    #[test]
    fn ignores_incomplete_or_loopback_only_advertisements() {
        let id = "c".repeat(64);
        assert!(parse(&advertisement("No API", &[("id", &id)])).is_none());
        assert!(parse(&advertisement("No ID", &[("api", "1")])).is_none());

        let loopback = ServiceInfo::new(
            SERVICE_TYPE,
            "Loopback",
            "loopback.local.",
            "127.0.0.1",
            8080,
            &[("id", id.as_str()), ("api", "1")][..],
        )
        .unwrap();
        assert!(parse(&loopback).is_none());
    }

    #[test]
    fn drops_an_invalid_public_url_but_keeps_the_lan_address() {
        let id = "d".repeat(64);
        let info = advertisement(
            "Backyard",
            &[
                ("id", &id),
                ("api", "1"),
                ("url", "http://public.example.com"),
            ],
        );
        let server = parse(&info).unwrap();
        assert!(server.remote_url.is_none());
        assert_eq!(server.lan_url, "http://192.168.1.44:8080/");
    }

    #[test]
    fn merges_interfaces_and_keeps_the_better_address() {
        let mut found = BTreeMap::new();
        let id = "e".repeat(64);
        insert_server(
            &mut found,
            DiscoveredServer {
                id: id.clone(),
                name: "Old".into(),
                lan_url: "http://203.0.113.9:8080/".into(),
                remote_url: None,
                api_version: 1,
                version: None,
            },
        );
        insert_server(
            &mut found,
            DiscoveredServer {
                id: id.clone(),
                name: "New".into(),
                lan_url: "http://192.168.1.44:8080/".into(),
                remote_url: Some("https://media.example.com/".into()),
                api_version: 1,
                version: Some("0.5.0".into()),
            },
        );
        let server = found.get(&id).unwrap();
        assert_eq!(server.lan_url, "http://192.168.1.44:8080/");
        assert_eq!(server.name, "New");
        assert_eq!(
            server.remote_url.as_deref(),
            Some("https://media.example.com/")
        );
        assert_eq!(server.version.as_deref(), Some("0.5.0"));
    }

    #[test]
    fn discovery_errors_are_readable() {
        let error = DiscoveryError::Mdns(mdns_sd::Error::Msg("no multicast".into()));
        assert!(error.to_string().contains("no multicast"));
    }
}
