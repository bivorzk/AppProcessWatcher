use std::{net::{IpAddr, SocketAddr}, path::PathBuf};
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkProtocol {
    TCP,
    UDP,
    HTTP,
    HTTPS,
    TLS,
    QUIC,
    UNKNOWN,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionDirection {
    Inbound,
    Outbound,
}

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

pub struct NetworkConnection {
    pub id: u64,
    pub process: Option<ProcessInfo>,
    pub protocol: NetworkProtocol,
    pub direction: ConnectionDirection,

    pub local_ip: IpAddr,
    pub local_port: u16,

    pub remote_ip: IpAddr,
    pub remote_port: u16,

    pub hostname: Option<String>,

    pub bytes_sent: u64,
    pub bytes_received: u64,
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