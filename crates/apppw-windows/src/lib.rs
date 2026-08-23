use std::{
    collections::HashSet,
    ffi::c_void,
    mem::size_of,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    process::Command,
};

use apppw_core::{NetworkConnection, NetworkProtocol, ProcessInfo};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HWND, LPARAM, NO_ERROR},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, HGDIOBJ, SelectObject,
        },
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID,
            MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
            MIB_UDP6ROW_OWNER_PID, MIB_UDP6TABLE_OWNER_PID, MIB_UDPROW_OWNER_PID,
            MIB_UDPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
        },
        Networking::WinSock::{AF_INET, AF_INET6},
        Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_ACCESS_RIGHTS, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess,
                WaitForSingleObject,
            },
        },
        UI::{
            Shell::{SHFILEINFOW, SHGFI_ICON, SHGetFileInfoW},
            WindowsAndMessaging::{
                DI_NORMAL, DestroyIcon, DrawIconEx, EnumWindows, GetWindowTextLengthW,
                GetWindowThreadProcessId, IsWindowVisible,
            },
        },
    },
    core::{BOOL, PCWSTR, PWSTR, Result as WindowsResult},
};

pub struct ProcessCollector;
pub struct SocketProcessResolver;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RelaunchSummary {
    pub relaunched: usize,
    pub failed: usize,
}

pub const APPLICATION_ICON_SIZE: usize = 32;

impl ProcessCollector {
    pub fn list_processes(&self) -> WindowsResult<Vec<ProcessInfo>> {
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
            NetworkProtocol::UDP | NetworkProtocol::QUIC => udp_pid(connection),
            _ => None,
        }
    }
}

pub fn relaunch_through_proxy(pid: u32, proxy: &str) -> Result<(), String> {
    if pid == std::process::id() {
        return Err("AppWatch cannot relaunch itself".into());
    }
    let path =
        process_image_path(pid).ok_or_else(|| "executable path is unavailable".to_owned())?;
    let process = unsafe {
        OpenProcess(
            PROCESS_TERMINATE | PROCESS_ACCESS_RIGHTS(0x0010_0000),
            false,
            pid,
        )
    }
    .map_err(|error| format!("could not open process: {error}"))?;
    let termination = unsafe { TerminateProcess(process, 0) };
    if termination.is_ok() {
        unsafe { WaitForSingleObject(process, 5_000) };
    }
    let _ = unsafe { CloseHandle(process) };
    termination.map_err(|error| format!("could not stop process: {error}"))?;

    let mut command = Command::new(&path);
    if let Some(directory) = path.parent() {
        command.current_dir(directory);
    }
    apply_proxy_environment(&mut command, proxy)
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", path.display()))?;
    Ok(())
}

fn apply_proxy_environment<'a>(command: &'a mut Command, proxy: &str) -> &'a mut Command {
    command.env("HTTP_PROXY", proxy).env("HTTPS_PROXY", proxy)
}

pub fn relaunch_open_apps_through_proxy(proxy: &str) -> Result<RelaunchSummary, String> {
    let mut pids = open_application_pids()?;
    pids.remove(&std::process::id());

    let mut summary = RelaunchSummary::default();
    for pid in pids {
        match relaunch_through_proxy(pid, proxy) {
            Ok(()) => summary.relaunched += 1,
            Err(_) => summary.failed += 1,
        }
    }
    Ok(summary)
}

pub fn open_application_pids() -> Result<HashSet<u32>, String> {
    let mut pids = HashSet::new();
    unsafe {
        EnumWindows(
            Some(collect_application_pid),
            LPARAM(&mut pids as *mut _ as isize),
        )
    }
    .map_err(|error| format!("could not enumerate open applications: {error}"))?;
    Ok(pids)
}

pub fn application_icon(path: &std::path::Path) -> Option<Vec<u8>> {
    let path = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut info = SHFILEINFOW::default();
    if unsafe {
        SHGetFileInfoW(
            PCWSTR(path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES::default(),
            Some(&mut info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON,
        )
    } == 0
    {
        return None;
    }

    let header = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: APPLICATION_ICON_SIZE as i32,
        biHeight: -(APPLICATION_ICON_SIZE as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let bitmap_info = BITMAPINFO {
        bmiHeader: header,
        ..Default::default()
    };
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        let _ = unsafe { DestroyIcon(info.hIcon) };
        return None;
    }
    let mut pixels = std::ptr::null_mut();
    let bitmap =
        unsafe { CreateDIBSection(Some(dc), &bitmap_info, DIB_RGB_COLORS, &mut pixels, None, 0) };
    let result = bitmap.ok().and_then(|bitmap| {
        let previous = unsafe { SelectObject(dc, HGDIOBJ(bitmap.0)) };
        let drawn = unsafe {
            DrawIconEx(
                dc,
                0,
                0,
                info.hIcon,
                APPLICATION_ICON_SIZE as i32,
                APPLICATION_ICON_SIZE as i32,
                0,
                None,
                DI_NORMAL,
            )
        };
        unsafe { SelectObject(dc, previous) };
        let rgba = drawn.ok().map(|()| {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    pixels.cast::<u8>(),
                    APPLICATION_ICON_SIZE * APPLICATION_ICON_SIZE * 4,
                )
            };
            bytes
                .chunks_exact(4)
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
                .collect()
        });
        let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        rgba
    });
    unsafe {
        let _ = DeleteDC(dc);
        let _ = DestroyIcon(info.hIcon);
    }
    result
}

unsafe extern "system" fn collect_application_pid(window: HWND, data: LPARAM) -> BOOL {
    if unsafe { IsWindowVisible(window).as_bool() && GetWindowTextLengthW(window) > 0 } {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
        if pid != 0 {
            let pids = unsafe { &mut *(data.0 as *mut HashSet<u32>) };
            pids.insert(pid);
        }
    }
    true.into()
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

    #[test]
    fn relaunch_command_sets_common_proxy_environment_names() {
        let mut command = Command::new("example.exe");
        apply_proxy_environment(&mut command, "http://127.0.0.1:8877");
        let variables = command
            .get_envs()
            .filter_map(|(name, value)| Some((name.to_str()?, value?.to_str()?)))
            .collect::<std::collections::HashMap<_, _>>();

        for name in ["HTTP_PROXY", "HTTPS_PROXY"] {
            assert_eq!(variables.get(name), Some(&"http://127.0.0.1:8877"));
        }
    }

    #[test]
    fn extracts_an_rgba_icon_for_an_executable() {
        let icon = application_icon(&std::env::current_exe().unwrap()).unwrap();
        assert_eq!(
            icon.len(),
            APPLICATION_ICON_SIZE * APPLICATION_ICON_SIZE * 4
        );
    }
}
