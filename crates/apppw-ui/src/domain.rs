#[derive(Clone)]
pub struct Process {
    pub pid: u32,
    pub name: &'static str,
    pub active: bool,
}

#[derive(Clone, Copy, PartialEq)]
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
    pub pid: u32,
    pub process: &'static str,
    pub method: &'static str,
    pub host: &'static str,
    pub path: &'static str,
    pub status: Option<u16>,
    pub kind: EventKind,
    pub sent: &'static str,
    pub received: &'static str,
    pub duration_ms: Option<u64>,
    pub local: &'static str,
    pub remote: &'static str,
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
