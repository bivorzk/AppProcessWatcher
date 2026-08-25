use std::fmt;
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkProtocol {
    TCP,
    UDP,
    HTTP,
    HTTPS,
    TLS,
    QUIC,
    UNKNOWN,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IpVersion {
    V4,
    V6,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub executable_path: Option<PathBuf>,
}

impl ProcessInfo {
    pub fn new(pid: u32, name: String, executable_path: Option<PathBuf>) -> Self {
        Self {
            pid,
            name,
            executable_path,
        }
    }
}

impl fmt::Display for ProcessInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [PID: {}]", self.name, self.pid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkConnection {
    pub id: u64,
    pub process: Option<ProcessInfo>,
    pub protocol: NetworkProtocol,
    pub ip_version: IpVersion,
    pub direction: ConnectionDirection,

    pub local_ip: IpAddr,
    pub local_port: u16,

    pub remote_ip: IpAddr,
    pub remote_port: u16,

    pub hostname: Option<String>,

    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedPacket {
    pub timestamp: SystemTime,
    pub protocol: NetworkProtocol,
    pub ip_version: IpVersion,
    pub direction: ConnectionDirection,
    pub source_ip: IpAddr,
    pub source_port: Option<u16>,
    pub destination_ip: IpAddr,
    pub destination_port: Option<u16>,
    pub packet_size: usize,
    pub payload_size: usize,
    pub process: Option<ProcessInfo>,
    pub raw_packet: Option<Vec<u8>>,
    pub tcp_fin: bool,
    pub tcp_rst: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkEvent {
    PacketCaptured(CapturedPacket),
    ConnectionOpened(NetworkConnection),
    ConnectionUpdated(NetworkConnection),
    ConnectionClosed(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequestInfo {
    pub id: u64,
    pub connection_id: Option<u64>,
    pub process: Option<ProcessInfo>,
    pub method: String,
    pub scheme: String,
    pub host: String,
    pub path: String,
    pub request_headers: Vec<HttpHeader>,
    pub request_body: Option<Vec<u8>>,
    pub request_body_size: usize,
    pub status_code: Option<u16>,
    pub response_headers: Vec<HttpHeader>,
    pub response_body: Option<Vec<u8>>,
    pub response_body_size: Option<usize>,
    pub started_at: SystemTime,
    pub duration_ms: Option<u128>,
    pub ja4: Option<String>,
}

impl HttpRequestInfo {
    pub fn display_url(&self) -> String {
        format!("{}://{}{}", self.scheme, self.host, self.path)
    }
}

pub fn redact_headers(headers: &[HttpHeader]) -> Vec<HttpHeader> {
    const SENSITIVE: [&str; 4] = [
        "authorization",
        "proxy-authorization",
        "cookie",
        "set-cookie",
    ];

    headers
        .iter()
        .map(|header| HttpHeader {
            name: header.name.clone(),
            value: if SENSITIVE
                .iter()
                .any(|name| header.name.eq_ignore_ascii_case(name))
            {
                "[REDACTED]".into()
            } else {
                header.value.clone()
            },
        })
        .collect()
}

struct ObservedConnection {
    connection: NetworkConnection,
    observed_at: SystemTime,
}

pub struct TrafficCorrelator {
    time_window: Duration,
    connections: Vec<ObservedConnection>,
}

impl TrafficCorrelator {
    pub fn new(time_window: Duration) -> Self {
        Self {
            time_window,
            connections: Vec::new(),
        }
    }

    pub fn observe_connection(&mut self, connection: NetworkConnection, observed_at: SystemTime) {
        if let Some(existing) = self
            .connections
            .iter_mut()
            .find(|item| item.connection.id == connection.id)
        {
            existing.connection = connection;
            existing.observed_at = observed_at;
        } else {
            self.connections.push(ObservedConnection {
                connection,
                observed_at,
            });
        }
    }

    pub fn correlate_http_request(
        &self,
        request: &mut HttpRequestInfo,
        protocol: NetworkProtocol,
        local: SocketAddr,
        remote: SocketAddr,
        timestamp: SystemTime,
    ) -> bool {
        let mut matches = self.connections.iter().filter(|item| {
            item.connection.protocol == protocol
                && item.connection.local_ip == local.ip()
                && item.connection.local_port == local.port()
                && item.connection.remote_ip == remote.ip()
                && item.connection.remote_port == remote.port()
                && item.connection.process.is_some()
                && time_difference(item.observed_at, timestamp) <= self.time_window
        });
        let Some(matched) = matches.next() else {
            return false;
        };
        if matches.next().is_some() {
            return false;
        }

        request.connection_id = Some(matched.connection.id);
        request.process = matched.connection.process.clone();
        true
    }
}

impl Default for TrafficCorrelator {
    fn default() -> Self {
        Self::new(Duration::from_secs(2))
    }
}

fn time_difference(left: SystemTime, right: SystemTime) -> Duration {
    left.duration_since(right)
        .or_else(|_| right.duration_since(left))
        .unwrap_or_default()
}

impl NetworkConnection {
    pub fn remote_address(&self) -> String {
        SocketAddr::new(self.remote_ip, self.remote_port).to_string()
    }

    pub fn display_destination(&self) -> String {
        if let Some(hostname) = &self.hostname {
            format!("{}:{}", hostname, self.remote_port)
        } else {
            self.remote_address()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn process_and_destination_formatting_are_stable() {
        let process = ProcessInfo::new(14_280, "Discord.exe".into(), None);
        assert_eq!(process.to_string(), "Discord.exe [PID: 14280]");

        let mut connection = NetworkConnection {
            id: 1,
            process: Some(process),
            protocol: NetworkProtocol::TCP,
            ip_version: IpVersion::V4,
            direction: ConnectionDirection::Outbound,
            local_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            local_port: 50_000,
            remote_ip: IpAddr::V6(Ipv6Addr::LOCALHOST),
            remote_port: 443,
            hostname: None,
            bytes_sent: 0,
            bytes_received: 0,
        };

        assert_eq!(connection.remote_address(), "[::1]:443");
        connection.hostname = Some("discord.com".into());
        assert_eq!(connection.display_destination(), "discord.com:443");
    }

    #[test]
    fn redaction_and_unique_correlation_do_not_guess() {
        let headers = vec![
            HttpHeader {
                name: "Authorization".into(),
                value: "secret".into(),
            },
            HttpHeader {
                name: "Accept".into(),
                value: "application/json".into(),
            },
        ];
        let redacted = redact_headers(&headers);
        assert_eq!(redacted[0].value, "[REDACTED]");
        assert_eq!(headers[0].value, "secret");

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let process = ProcessInfo::new(42, "client.exe".into(), None);
        let connection = NetworkConnection {
            id: 7,
            process: Some(process.clone()),
            protocol: NetworkProtocol::TCP,
            ip_version: IpVersion::V4,
            direction: ConnectionDirection::Outbound,
            local_ip: "127.0.0.1".parse().unwrap(),
            local_port: 50_000,
            remote_ip: "93.184.216.34".parse().unwrap(),
            remote_port: 80,
            hostname: Some("example.com".into()),
            bytes_sent: 100,
            bytes_received: 200,
        };
        let mut correlator = TrafficCorrelator::default();
        correlator.observe_connection(connection.clone(), now);
        let mut request = test_request(now);

        assert!(correlator.correlate_http_request(
            &mut request,
            NetworkProtocol::TCP,
            "127.0.0.1:50000".parse().unwrap(),
            "93.184.216.34:80".parse().unwrap(),
            now,
        ));
        assert_eq!(request.connection_id, Some(7));
        assert_eq!(request.process, Some(process));

        let mut duplicate = connection;
        duplicate.id = 8;
        correlator.observe_connection(duplicate, now);
        let mut ambiguous_request = test_request(now);
        assert!(!correlator.correlate_http_request(
            &mut ambiguous_request,
            NetworkProtocol::TCP,
            "127.0.0.1:50000".parse().unwrap(),
            "93.184.216.34:80".parse().unwrap(),
            now,
        ));
        assert!(ambiguous_request.process.is_none());
    }

    fn test_request(started_at: SystemTime) -> HttpRequestInfo {
        HttpRequestInfo {
            id: 1,
            connection_id: None,
            process: None,
            method: "GET".into(),
            scheme: "http".into(),
            host: "example.com".into(),
            path: "/".into(),
            request_headers: Vec::new(),
            request_body: None,
            request_body_size: 0,
            status_code: None,
            response_headers: Vec::new(),
            response_body: None,
            response_body_size: None,
            started_at,
            duration_ms: None,
            ja4: None,
        }
    }
}
