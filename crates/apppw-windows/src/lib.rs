use std::{
    mem::size_of,
    net::{IpAddr},
    path::PathBuf,
};

use apppw_core::{NetworkConnection, NetworkProtocol, ProcessInfo};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, NO_ERROR}, NetworkManagement::IpHelper::{
            GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
            MIB_UDPROW_OWNER_PID, MIB_UDPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
            UDP_TABLE_OWNER_PID,
        }, Networking::WinSock::AF_INET, System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
    }, core::{PWSTR, Result},
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
                let name = String::from_utf16_lossy(&entry.szExeFile[..name_length]);
                processes.push(ProcessInfo::new(pid, name, process_image_path(pid)));

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
        if connection.protocol == NetworkProtocol::TCP {
            let mut table_size = 0;
            let result = unsafe {
                GetExtendedTcpTable(
                    None,
                    &mut table_size,
                    false,
                    AF_INET.0 as u32,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            };

            if result != NO_ERROR.0 && result != windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER.0 {
                return None;
            }

            let mut buffer = vec![0_u8; table_size as usize];
            let result = unsafe {
                GetExtendedTcpTable(
                    Some(buffer.as_mut_ptr() as *mut _),
                    &mut table_size,
                    false,
                    AF_INET.0 as u32,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            };

            if result != NO_ERROR.0 {
                return None;
            }

            let tcp_table: &MIB_TCPTABLE_OWNER_PID =
                unsafe { &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) };

                let IpAddr::V4(local_ip) = connection.local_ip else {
                return None;
            };
            let IpAddr::V4(remote_ip) = connection.remote_ip else {
                return None;
            };

            for i in 0..tcp_table.dwNumEntries {
                let row: &MIB_TCPROW_OWNER_PID =
                    unsafe { &*tcp_table.table.as_ptr().add(i as usize) };

                if row.dwLocalAddr
                    == u32::from_ne_bytes(local_ip.octets())
                    && row.dwLocalPort == u32::from(connection.local_port.to_be())
                    && row.dwRemoteAddr == u32::from_ne_bytes(remote_ip.octets())
                    && row.dwRemotePort == u32::from(connection.remote_port.to_be())
                {
                    return process_for_pid(row.dwOwningPid);
                }
            }

            fn process_for_pid(pid: u32) -> Option<ProcessInfo> {
            ProcessCollector
                .list_processes()
                .ok()?
                .into_iter()
                .find(|process| process.pid == pid)
        }
        }

        if connection.protocol == NetworkProtocol::UDP {
            let mut table_size = 0;
            let result = unsafe {
                GetExtendedUdpTable(
                    None,
                    &mut table_size,
                    false,
                    AF_INET.0 as u32,
                    UDP_TABLE_OWNER_PID,
                    0,
                )
            };

            if result != NO_ERROR.0 && result != ERROR_INSUFFICIENT_BUFFER.0 {
                return None;
            }

            let mut buffer = vec![0_u8; table_size as usize];
            let result = unsafe {
                GetExtendedUdpTable(
                    Some(buffer.as_mut_ptr() as *mut _),
                    &mut table_size,
                    false,
                    AF_INET.0 as u32,
                    UDP_TABLE_OWNER_PID,
                    0,
                )
            };

            if result != NO_ERROR.0 {
                return None;
            }

            let udp_table: &MIB_UDPTABLE_OWNER_PID =
                unsafe { &*(buffer.as_ptr() as *const MIB_UDPTABLE_OWNER_PID) };
                        
            let IpAddr::V4(local_ip) = connection.local_ip else {
                return None;
            };
            for i in 0..udp_table.dwNumEntries {
                let row: &MIB_UDPROW_OWNER_PID =
                    unsafe { &*udp_table.table.as_ptr().add(i as usize) };

                if row.dwLocalAddr
                    == u32::from_ne_bytes(
                        local_ip.octets(),
                    )
                    && row.dwLocalPort == u32::from(connection.local_port.to_be())
                {
                    return process_for_pid(row.dwOwningPid);
                }
            }

            fn process_for_pid(pid: u32) -> Option<ProcessInfo> {
            ProcessCollector
                .list_processes()
                .ok()?
                .into_iter()
                .find(|process| process.pid == pid)
        }

        }
        
        None
    }
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
