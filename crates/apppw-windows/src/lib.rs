use std::{
    collections::HashSet,
    ffi::c_void,
    mem::size_of,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
};

use apppw_core::{NetworkConnection, NetworkProtocol, ProcessInfo};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, NO_ERROR},
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID,
            MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
            MIB_UDP6ROW_OWNER_PID, MIB_UDP6TABLE_OWNER_PID, MIB_UDPROW_OWNER_PID,
            MIB_UDPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
        },
        Networking::WinSock::{AF_INET, AF_INET6},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
    },
    core::{PWSTR, Result},
};

pub struct ProcessCollector;
pub struct SocketProcessResolver;

impl ProcessCollector {
    pub fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? };
        let result = (|| {
            let mut entry = PROCESSENTRY32W {
                dwSize: size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut processes = Vec::new();
            unsafe { Process32FirstW(snapshot, &mut entry)? };

            loop {
                let name_length = entry
                    .szExeFile
                    .iter()
                    .position(|character| *character == 0)
                    .unwrap_or(entry.szExeFile.len());
                let pid = entry.th32ProcessID;
                processes.push(ProcessInfo::new(
                    pid,
                    String::from_utf16_lossy(&entry.szExeFile[..name_length]),
                    process_image_path(pid),
                ));

                if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                    break;
                }
            }
            Ok(processes)
        })();

        unsafe { CloseHandle(snapshot)? };
        result
    }
}

impl SocketProcessResolver {
    pub fn resolve(&self, connection: &NetworkConnection) -> Option<ProcessInfo> {
        let pid = self.resolve_pid(connection)?;
        ProcessCollector
            .list_processes()
            .ok()?
            .into_iter()
            .find(|process| process.pid == pid)
    }

    pub fn resolve_pid(&self, connection: &NetworkConnection) -> Option<u32> {
        match connection.protocol {
            NetworkProtocol::TCP => tcp_pid(connection),
            NetworkProtocol::UDP => udp_pid(connection),
            _ => None,
        }
    }
}

fn tcp_pid(connection: &NetworkConnection) -> Option<u32> {
    match (connection.local_ip, connection.remote_ip) {
        (IpAddr::V4(local), IpAddr::V4(remote)) => {
            let buffer = tcp_table(AF_INET.0 as u32)?;
            let table = unsafe { &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) };
            rows(&table.table, table.dwNumEntries).find_map(|row: &MIB_TCPROW_OWNER_PID| {
                (ipv4(row.dwLocalAddr) == local
                    && port(row.dwLocalPort) == connection.local_port
                    && ipv4(row.dwRemoteAddr) == remote
                    && port(row.dwRemotePort) == connection.remote_port)
                    .then_some(row.dwOwningPid)
            })
        }
        (IpAddr::V6(local), IpAddr::V6(remote)) => {
            let buffer = tcp_table(AF_INET6.0 as u32)?;
            let table = unsafe { &*(buffer.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID) };
            rows(&table.table, table.dwNumEntries).find_map(|row: &MIB_TCP6ROW_OWNER_PID| {
                (Ipv6Addr::from(row.ucLocalAddr) == local
                    && port(row.dwLocalPort) == connection.local_port
                    && Ipv6Addr::from(row.ucRemoteAddr) == remote
                    && port(row.dwRemotePort) == connection.remote_port)
                    .then_some(row.dwOwningPid)
            })
        }
        _ => None,
    }
}

fn udp_pid(connection: &NetworkConnection) -> Option<u32> {
    let pids: HashSet<u32> = match connection.local_ip {
        IpAddr::V4(local) => {
            let buffer = udp_table(AF_INET.0 as u32)?;
            let table = unsafe { &*(buffer.as_ptr() as *const MIB_UDPTABLE_OWNER_PID) };
            rows(&table.table, table.dwNumEntries)
                .filter(|row: &&MIB_UDPROW_OWNER_PID| {
                    let address = ipv4(row.dwLocalAddr);
                    (address.is_unspecified() || address == local)
                        && port(row.dwLocalPort) == connection.local_port
                })
                .map(|row| row.dwOwningPid)
                .collect()
        }
        IpAddr::V6(local) => {
            let buffer = udp_table(AF_INET6.0 as u32)?;
            let table = unsafe { &*(buffer.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID) };
            rows(&table.table, table.dwNumEntries)
                .filter(|row: &&MIB_UDP6ROW_OWNER_PID| {
                    let address = Ipv6Addr::from(row.ucLocalAddr);
                    (address.is_unspecified() || address == local)
                        && port(row.dwLocalPort) == connection.local_port
                })
                .map(|row| row.dwOwningPid)
                .collect()
        }
    };

    (pids.len() == 1).then(|| pids.into_iter().next()).flatten()
}

fn tcp_table(family: u32) -> Option<Vec<usize>> {
    query_table(|buffer, size| unsafe {
        GetExtendedTcpTable(buffer, size, false, family, TCP_TABLE_OWNER_PID_ALL, 0)
    })
}

fn udp_table(family: u32) -> Option<Vec<usize>> {
    query_table(|buffer, size| unsafe {
        GetExtendedUdpTable(buffer, size, false, family, UDP_TABLE_OWNER_PID, 0)
    })
}

fn query_table(mut query: impl FnMut(Option<*mut c_void>, &mut u32) -> u32) -> Option<Vec<usize>> {
    let mut byte_count = 0;
    let first = query(None, &mut byte_count);
    if first != NO_ERROR.0 && first != ERROR_INSUFFICIENT_BUFFER.0 {
        return None;
    }

    let word_size = size_of::<usize>();
    let mut buffer = vec![0_usize; (byte_count as usize).div_ceil(word_size)];
    (query(Some(buffer.as_mut_ptr().cast::<c_void>()), &mut byte_count) == NO_ERROR.0)
        .then_some(buffer)
}

fn rows<T>(first: &[T; 1], count: u32) -> impl Iterator<Item = &T> {
    unsafe { std::slice::from_raw_parts(first.as_ptr(), count as usize) }.iter()
}

fn ipv4(raw: u32) -> Ipv4Addr {
    Ipv4Addr::from(raw.to_ne_bytes())
}

fn port(raw: u32) -> u16 {
    u16::from_be(raw as u16)
}

fn process_image_path(pid: u32) -> Option<PathBuf> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    let mut path = vec![0_u16; 32_768];
    let mut path_length = path.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            Default::default(),
            PWSTR(path.as_mut_ptr()),
            &mut path_length,
        )
    };
    unsafe { CloseHandle(process).ok()? };
    result.ok()?;
    Some(PathBuf::from(String::from_utf16_lossy(
        &path[..path_length as usize],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_windows_network_order_values() {
        assert_eq!(port(u16::to_be(443) as u32), 443);
        assert_eq!(
            ipv4(u32::from_ne_bytes([192, 168, 1, 10])),
            Ipv4Addr::new(192, 168, 1, 10)
        );
    }

    #[test]
    fn process_collector_finds_the_current_process() {
        let current_pid = std::process::id();
        let processes = ProcessCollector.list_processes().unwrap();

        assert!(processes.iter().any(|process| process.pid == current_pid));
    }
}
