use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, SystemTime},
};

use apppw_core::{
    CapturedPacket, ConnectionDirection, IpVersion, NetworkConnection, NetworkEvent,
    NetworkProtocol,
};
use etherparse::{InternetSlice, SlicedPacket, TransportSlice};
use windivert::prelude::{WinDivert, WinDivertFlags};

pub const CONNECTION_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(30);

pub struct PacketCapture;

impl PacketCapture {
    pub fn start() -> Result<Receiver<NetworkEvent>, Box<dyn std::error::Error>> {
        Self::start_with_raw_packets(false)
    }

    pub fn start_with_raw_packets(
        retain_raw_packets: bool,
    ) -> Result<Receiver<NetworkEvent>, Box<dyn std::error::Error>> {
        let flags = WinDivertFlags::new().set_sniff();
        let handle = WinDivert::network("(tcp or udp)", 0, flags)?;
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            loop {
                let packet = match handle.recv(None) {
                    Ok(packet) => packet,
                    Err(_) => break,
                };
                if let Some(packet) = parse_packet(
                    packet.data.as_ref(),
                    packet.address.outbound(),
                    retain_raw_packets,
                ) && sender.send(NetworkEvent::PacketCaptured(packet)).is_err()
                {
                    break;
                }
            }
        });

        Ok(receiver)
    }
}

pub fn parse_packet(
    data: &[u8],
    outbound: bool,
    retain_raw_packet: bool,
) -> Option<CapturedPacket> {
    let packet = SlicedPacket::from_ip(data).ok()?;
    let (source_ip, destination_ip, ip_version) = match packet.ip? {
        InternetSlice::Ipv4(header, _) => (
            IpAddr::V4(Ipv4Addr::from(header.source())),
            IpAddr::V4(Ipv4Addr::from(header.destination())),
            IpVersion::V4,
        ),
        InternetSlice::Ipv6(header, _) => (
            IpAddr::V6(Ipv6Addr::from(header.source())),
            IpAddr::V6(Ipv6Addr::from(header.destination())),
            IpVersion::V6,
        ),
    };
    let (source_port, destination_port, protocol, tcp_fin, tcp_rst) = match packet.transport? {
        TransportSlice::Tcp(header) => (
            header.source_port(),
            header.destination_port(),
            NetworkProtocol::TCP,
            header.fin(),
            header.rst(),
        ),
        TransportSlice::Udp(header) => (
            header.source_port(),
            header.destination_port(),
            NetworkProtocol::UDP,
            false,
            false,
        ),
        _ => return None,
    };

    Some(CapturedPacket {
        timestamp: SystemTime::now(),
        protocol,
        ip_version,
        direction: if outbound {
            ConnectionDirection::Outbound
        } else {
            ConnectionDirection::Inbound
        },
        source_ip,
        source_port: Some(source_port),
        destination_ip,
        destination_port: Some(destination_port),
        packet_size: data.len(),
        payload_size: packet.payload.len(),
        process: None,
        raw_packet: retain_raw_packet.then(|| data.to_vec()),
        tcp_fin,
        tcp_rst,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConnectionKey {
    protocol: NetworkProtocol,
    local_ip: IpAddr,
    local_port: u16,
    remote_ip: IpAddr,
    remote_port: u16,
}

struct TrackedConnection {
    connection: NetworkConnection,
    _started_at: SystemTime,
    last_seen: SystemTime,
}

pub struct ConnectionTracker {
    connections: HashMap<ConnectionKey, TrackedConnection>,
    next_id: u64,
}

impl Default for ConnectionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionTracker {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn process_packet(&mut self, packet: CapturedPacket) -> Vec<NetworkEvent> {
        let mut events = vec![NetworkEvent::PacketCaptured(packet.clone())];
        events.extend(self.close_inactive(packet.timestamp));

        let Some(key) = ConnectionKey::from_packet(&packet) else {
            return events;
        };
        let size = packet.packet_size as u64;
        let is_outbound = packet.direction == ConnectionDirection::Outbound;
        let is_closed =
            packet.protocol == NetworkProtocol::TCP && (packet.tcp_fin || packet.tcp_rst);

        if let Some(tracked) = self.connections.get_mut(&key) {
            tracked.last_seen = packet.timestamp;
            if tracked.connection.process.is_none() {
                tracked.connection.process = packet.process.clone();
            }
            if is_outbound {
                tracked.connection.bytes_sent = tracked.connection.bytes_sent.saturating_add(size);
            } else {
                tracked.connection.bytes_received =
                    tracked.connection.bytes_received.saturating_add(size);
            }
            events.push(NetworkEvent::ConnectionUpdated(tracked.connection.clone()));
        } else {
            let (local_ip, local_port, remote_ip, remote_port) = key.endpoints();
            let connection = NetworkConnection {
                id: self.next_id,
                process: packet.process.clone(),
                protocol: packet.protocol,
                ip_version: packet.ip_version,
                direction: packet.direction,
                local_ip,
                local_port,
                remote_ip,
                remote_port,
                hostname: None,
                bytes_sent: if is_outbound { size } else { 0 },
                bytes_received: if is_outbound { 0 } else { size },
            };
            self.next_id = self.next_id.saturating_add(1);
            self.connections.insert(
                key.clone(),
                TrackedConnection {
                    connection: connection.clone(),
                    _started_at: packet.timestamp,
                    last_seen: packet.timestamp,
                },
            );
            events.push(NetworkEvent::ConnectionOpened(connection));
        }

        if is_closed && let Some(closed) = self.connections.remove(&key) {
            events.push(NetworkEvent::ConnectionClosed(closed.connection.id));
        }
        events
    }

    pub fn close_inactive(&mut self, now: SystemTime) -> Vec<NetworkEvent> {
        let expired: Vec<_> = self
            .connections
            .iter()
            .filter(|(_, tracked)| {
                now.duration_since(tracked.last_seen).unwrap_or_default()
                    >= CONNECTION_INACTIVITY_TIMEOUT
            })
            .map(|(key, tracked)| (key.clone(), tracked.connection.id))
            .collect();

        for (key, _) in &expired {
            self.connections.remove(key);
        }
        expired
            .into_iter()
            .map(|(_, id)| NetworkEvent::ConnectionClosed(id))
            .collect()
    }
}

impl ConnectionKey {
    fn from_packet(packet: &CapturedPacket) -> Option<Self> {
        let source_port = packet.source_port?;
        let destination_port = packet.destination_port?;
        let (local_ip, local_port, remote_ip, remote_port) = match packet.direction {
            ConnectionDirection::Outbound => (
                packet.source_ip,
                source_port,
                packet.destination_ip,
                destination_port,
            ),
            ConnectionDirection::Inbound => (
                packet.destination_ip,
                destination_port,
                packet.source_ip,
                source_port,
            ),
        };
        Some(Self {
            protocol: packet.protocol,
            local_ip,
            local_port,
            remote_ip,
            remote_port,
        })
    }

    fn endpoints(&self) -> (IpAddr, u16, IpAddr, u16) {
        (
            self.local_ip,
            self.local_port,
            self.remote_ip,
            self.remote_port,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use etherparse::PacketBuilder;

    #[test]
    fn parses_payload_without_retaining_raw_bytes() {
        let builder =
            PacketBuilder::ipv4([192, 168, 1, 10], [1, 1, 1, 1], 64).tcp(50_000, 443, 1, 1);
        let payload = b"hello";
        let mut bytes = Vec::with_capacity(builder.size(payload.len()));
        builder.write(&mut bytes, payload).unwrap();

        let packet = parse_packet(&bytes, true, false).unwrap();
        assert_eq!(packet.payload_size, payload.len());
        assert_eq!(packet.source_port, Some(50_000));
        assert_eq!(packet.destination_port, Some(443));
        assert!(packet.raw_packet.is_none());
    }

    #[test]
    fn tracker_merges_reply_and_counts_both_directions() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let outbound = test_packet(now, ConnectionDirection::Outbound, 100);
        let mut inbound = test_packet(
            now + Duration::from_secs(1),
            ConnectionDirection::Inbound,
            250,
        );
        std::mem::swap(&mut inbound.source_ip, &mut inbound.destination_ip);
        std::mem::swap(&mut inbound.source_port, &mut inbound.destination_port);

        let mut tracker = ConnectionTracker::new();
        assert!(matches!(
            tracker.process_packet(outbound)[1],
            NetworkEvent::ConnectionOpened(_)
        ));
        let events = tracker.process_packet(inbound);
        let NetworkEvent::ConnectionUpdated(connection) = &events[1] else {
            panic!("expected an updated connection");
        };
        assert_eq!(connection.bytes_sent, 100);
        assert_eq!(connection.bytes_received, 250);

        let closed =
            tracker.close_inactive(now + CONNECTION_INACTIVITY_TIMEOUT + Duration::from_secs(2));
        assert_eq!(closed, vec![NetworkEvent::ConnectionClosed(connection.id)]);
    }

    fn test_packet(
        timestamp: SystemTime,
        direction: ConnectionDirection,
        packet_size: usize,
    ) -> CapturedPacket {
        CapturedPacket {
            timestamp,
            protocol: NetworkProtocol::TCP,
            ip_version: IpVersion::V4,
            direction,
            source_ip: "192.168.1.10".parse().unwrap(),
            source_port: Some(50_000),
            destination_ip: "1.1.1.1".parse().unwrap(),
            destination_port: Some(443),
            packet_size,
            payload_size: 0,
            process: None,
            raw_packet: None,
            tcp_fin: false,
            tcp_rst: false,
        }
    }
}
