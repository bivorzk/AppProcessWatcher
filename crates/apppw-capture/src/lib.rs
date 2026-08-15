use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::mpsc::{self, Receiver},
    thread,
};

use apppw_core::{ConnectionDirection, NetworkProtocol};
use etherparse::{InternetSlice, SlicedPacket, TransportSlice};
use windivert::prelude::{WinDivert, WinDivertFlags};

pub struct PacketCapture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkEvent {
    pub source_ip: IpAddr,
    pub source_port: u16,
    pub destination_ip: IpAddr,
    pub destination_port: u16,
    pub protocol: NetworkProtocol,
    pub ip_version: IpVersion,
    pub direction: ConnectionDirection,
    pub packet_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpVersion {
    V4,
    V6,
}

impl PacketCapture {
    pub fn start() -> Result<Receiver<NetworkEvent>, Box<dyn std::error::Error>> {
        let flags = WinDivertFlags::new().set_sniff();
        let handle = WinDivert::network("(tcp or udp)", 0, flags)?;
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            loop {
                let packet = match handle.recv(None) {
                    Ok(packet) => packet,
                    Err(_) => break,
                };

                if let Some(event) =
                    NetworkEvent::from_packet(packet.data.as_ref(), packet.address.outbound())
                    && sender.send(event).is_err()
                {
                    break;
                }
            }
        });

        Ok(receiver)
    }
}

impl NetworkEvent {
    fn from_packet(data: &[u8], outbound: bool) -> Option<Self> {
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
        let (source_port, destination_port, protocol) = match packet.transport? {
            TransportSlice::Tcp(header) => (
                header.source_port(),
                header.destination_port(),
                NetworkProtocol::TCP,
            ),
            TransportSlice::Udp(header) => (
                header.source_port(),
                header.destination_port(),
                NetworkProtocol::UDP,
            ),
            _ => return None,
        };

        Some(Self {
            source_ip,
            source_port,
            destination_ip,
            destination_port,
            protocol,
            ip_version,
            direction: if outbound {
                ConnectionDirection::Outbound
            } else {
                ConnectionDirection::Inbound
            },
            packet_size: data.len(),
        })
    }
}
