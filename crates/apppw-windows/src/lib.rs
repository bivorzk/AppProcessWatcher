use std::mem::size_of;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
    PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

pub struct ProcessCollector;

fn list_process_names() -> std::result::Result<Vec<(u32, String)>, windows_sys::core::HRESULT> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == 0 || snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return Err(windows_sys::core::HRESULT::default());
    }

    let result = (|| {
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut processes = Vec::new();

        if unsafe { Process32FirstW(snapshot, &mut entry) } == 0 {
            return Ok(processes);
        }

        loop {
            let name_length = entry
                .szExeFile
                .iter()
                .position(|character| *character == 0)
                .unwrap_or(entry.szExeFile.len());

            let name = String::from_utf16_lossy(&entry.szExeFile[..name_length]);
            processes.push((entry.th32ProcessID, name));

            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }

        Ok(processes)
    })();

    unsafe {
        CloseHandle(snapshot);
    }
    result
}