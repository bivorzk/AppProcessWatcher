#[derive(Clone)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub active: bool,
    pub application: bool,
    pub icon_rgba: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    Http,
    Https,
    Tls,
    Tcp,
    Udp,
    Quic,
    Unknown,
}

impl EventKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Https => "HTTPS decoded",
            Self::Tls => "TLS encrypted",
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
            Self::Quic => "QUIC",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone)]
pub struct NetworkEvent {
    pub id: u64,
    pub pid: Option<u32>,
    pub process: String,
    pub method: String,
    pub host: String,
    pub path: String,
    pub status: Option<u16>,
    pub kind: EventKind,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packet_count: u64,
    pub duration_ms: Option<u64>,
    pub local: String,
    pub remote: String,
    pub request_headers: Vec<HttpHeader>,
    pub response_headers: Vec<HttpHeader>,
    pub request_body: Option<Vec<u8>>,
    pub response_body: Option<Vec<u8>>,
}

impl NetworkEvent {
    pub fn search_text(&self) -> String {
        format!(
            "{} {} {} {} {} {} {} {}",
            self.process,
            self.method,
            self.host,
            self.path,
            self.status
                .map_or_else(|| "---".into(), |value| value.to_string()),
            self.kind.label(),
            self.local,
            self.remote
        )
        .to_lowercase()
    }
}
use apppw_core::HttpHeader;
