#[path = "cpa_process.rs"]
mod process;
pub(crate) use process::{attach_job, executable, processes, spawn, stop};

use std::{
    ffi::{OsStr, OsString},
    fs, io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{LocalFree, ERROR_INSUFFICIENT_BUFFER, HWND},
    NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    },
    Networking::WinSock::{AF_INET, AF_INET6},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
        },
        Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
        GetSecurityDescriptorDacl, GetTokenInformation, TokenUser, DACL_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
    },
    Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives, GetVolumePathNameW, MoveFileW},
    System::{
        Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED},
        Threading::{GetCurrentProcess, OpenProcessToken},
        WindowsProgramming::{DRIVE_FIXED, DRIVE_RAMDISK, DRIVE_REMOVABLE},
    },
    UI::{
        Shell::{
            SHBrowseForFolderW, SHGetPathFromIDListEx, ShellExecuteW, BIF_NEWDIALOGSTYLE,
            BIF_RETURNONLYFSDIRS, BROWSEINFOW, GPFIDL_DEFAULT,
        },
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

pub(crate) fn canonical(path: &Path) -> io::Result<PathBuf> {
    dunce::canonicalize(path)
}

pub(crate) fn local_volume(path: &Path) -> bool {
    let mut root = vec![0; 32768];
    if unsafe { GetVolumePathNameW(wide(path).as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0
    {
        return false;
    }
    matches!(
        unsafe { GetDriveTypeW(root.as_ptr()) },
        DRIVE_FIXED | DRIVE_RAMDISK | DRIVE_REMOVABLE
    )
}

pub(crate) fn externally_managed(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name.eq_ignore_ascii_case("sidecar") || name.eq_ignore_ascii_case("cpa-core")
    })
}

pub(crate) fn tool(name: &str, defaults: &[(&str, &str)]) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .chain(defaults.iter().filter_map(|(variable, dir)| {
            std::env::var_os(variable).map(|root| PathBuf::from(root).join(dir))
        }))
        .filter(|directory| directory.is_absolute())
        .find_map(|directory| {
            canonical(&directory.join(name))
                .ok()
                .filter(|path| path.is_file())
        })
}

pub(crate) fn disk_candidates() -> Vec<PathBuf> {
    let mask = unsafe { GetLogicalDrives() };
    let mut result = Vec::new();
    for (index, letter) in ('A'..='Z').enumerate() {
        if mask & (1 << index) == 0 {
            continue;
        }
        let root = PathBuf::from(format!("{letter}:\\"));
        if unsafe { GetDriveTypeW(wide(&root).as_ptr()) } != DRIVE_FIXED {
            continue;
        }
        let paths = [
            "CLIProxy",
            "CLIProxyAPI",
            "Tools\\CLIProxy",
            "Program Files\\CLIProxyAPI",
        ];
        for path in paths.map(|path| root.join(path)) {
            for directory in [path.clone(), path.join(".dev")] {
                collect_cpa(&directory, &mut result);
            }
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten().filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(|c: char| c.is_ascii_digit())
                        && entry
                            .file_type()
                            .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink())
                }) {
                    collect_cpa(&entry.path(), &mut result);
                }
            }
        }
        collect_cpa(
            &root.join("CPSwitch/resources/sidecar/windows-x86_64"),
            &mut result,
        );
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten().filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .starts_with("easycliproxyapi")
            }) {
                collect_cpa(&entry.path().join("cpa-core"), &mut result);
            }
        }
    }
    for name in ["cli-proxy-api.exe", "cliproxyapi.exe", "cpa.exe"] {
        result.extend(tool(name, &[]));
    }
    result
}

fn collect_cpa(directory: &Path, result: &mut Vec<PathBuf>) {
    for name in [
        "cli-proxy-api.exe",
        "cliproxyapi.exe",
        "cpa.exe",
        "cli-proxy-api-plus.exe",
    ] {
        let path = directory.join(name);
        if path.is_file() {
            result.push(path);
        }
    }
}

pub(crate) fn listeners(port: u16) -> Result<Vec<u32>, String> {
    let mut owners = Vec::new();
    let matches_port = |value: u32| u16::from_be(value as u16) == port;
    for row in tcp_rows::<MIB_TCPROW_OWNER_PID>(AF_INET)? {
        let address = row.dwLocalAddr.to_ne_bytes();
        if matches_port(row.dwLocalPort) && (address == [0; 4] || address == [127, 0, 0, 1]) {
            owners.push(row.dwOwningPid);
        }
    }
    for row in tcp_rows::<MIB_TCP6ROW_OWNER_PID>(AF_INET6)? {
        let address = std::net::Ipv6Addr::from(row.ucLocalAddr);
        if matches_port(row.dwLocalPort)
            && (address.is_unspecified()
                || address.to_ipv4_mapped() == Some(std::net::Ipv4Addr::LOCALHOST))
        {
            owners.push(row.dwOwningPid);
        }
    }
    owners.sort_unstable();
    owners.dedup();
    Ok(owners)
}

fn tcp_rows<T: Copy>(family: u16) -> Result<Vec<T>, String> {
    let query = |data, size| unsafe {
        GetExtendedTcpTable(
            data,
            size,
            0,
            u32::from(family),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    let mut size = 0;
    let error = "无法核实 Windows 端口的监听进程";
    if query(null_mut(), &mut size) != ERROR_INSUFFICIENT_BUFFER {
        return Err(error.into());
    }
    for _ in 0..3 {
        let mut buffer = vec![0u32; (size as usize).div_ceil(4)];
        match query(buffer.as_mut_ptr().cast(), &mut size) {
            0 => {
                let count = buffer.first().copied().ok_or(error)? as usize;
                let bytes = (size as usize).min(buffer.len() * 4);
                if count > bytes.saturating_sub(4) / std::mem::size_of::<T>() {
                    return Err(error.into());
                }
                // Both owner tables contain DWORD-aligned rows after their DWORD entry count.
                let rows = unsafe {
                    std::slice::from_raw_parts(buffer.as_ptr().add(1).cast::<T>(), count)
                };
                return Ok(rows.to_vec());
            }
            ERROR_INSUFFICIENT_BUFFER => {}
            _ => break,
        }
    }
    Err(error.into())
}

pub(crate) fn choose_parent(hwnd: usize) -> Result<Option<String>, String> {
    if unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) } < 0 {
        return Err("无法初始化 Windows 目录选择器".into());
    }
    let title = wide("选择 CLIProxy 安装目录的上一级");
    let options = BROWSEINFOW {
        hwndOwner: hwnd as HWND,
        lpszTitle: title.as_ptr(),
        ulFlags: BIF_NEWDIALOGSTYLE | BIF_RETURNONLYFSDIRS,
        ..Default::default()
    };
    let item = unsafe { SHBrowseForFolderW(&options) };
    let mut buf = vec![0u16; 32768];
    let resolved = !item.is_null()
        && unsafe {
            SHGetPathFromIDListEx(item, buf.as_mut_ptr(), buf.len() as u32, GPFIDL_DEFAULT)
        } != 0;
    unsafe {
        CoTaskMemFree(item.cast());
        CoUninitialize();
    }
    if item.is_null() {
        return Ok(None);
    }
    if !resolved {
        return Err("无法读取所选目录".into());
    }
    buf.truncate(buf.iter().position(|&s| s == 0).unwrap_or(buf.len()));
    Ok(Some(
        PathBuf::from(OsString::from_wide(&buf))
            .join("CLIProxy")
            .to_string_lossy()
            .into_owned(),
    ))
}

pub(crate) fn open_browser(url: &str) -> Result<(), String> {
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    (result as usize > 32)
        .then_some(())
        .ok_or("CPA 管理页未能打开".into())
}

pub(crate) fn random(bytes: &mut [u8]) -> io::Result<()> {
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::other("BCryptGenRandom"))
    }
}

pub(crate) fn private_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    if !local_volume(path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Local volume required",
        ));
    }
    let mut raw_token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = process::handle(raw_token)?;
    let mut size = 0;
    unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut size) };
    let mut data = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
    if size == 0
        || unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                data.as_mut_ptr().cast(),
                size,
                &mut size,
            )
        } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let user = unsafe { &*data.as_ptr().cast::<TOKEN_USER>() };
    let mut sid = null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let sid_len = (0..).find(|&i| unsafe { *sid.add(i) == 0 }).unwrap();
    let sid_text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid, sid_len) });
    unsafe { LocalFree(sid.cast()) };
    let mut descriptor = null_mut();
    let sddl = wide(format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{sid_text})"));
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut present = 0;
    let mut defaulted = 0;
    let mut acl = null_mut();
    let result =
        if unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted) }
            == 0
            || present == 0
            || acl.is_null()
        {
            Err(io::Error::other("Missing file DACL"))
        } else {
            let status = unsafe {
                SetNamedSecurityInfoW(
                    wide(path).as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    acl,
                    null(),
                )
            };
            if status == 0 {
                Ok(())
            } else {
                Err(io::Error::from_raw_os_error(status as i32))
            }
        };
    unsafe { LocalFree(descriptor) };
    result
}

pub(crate) fn promote(from: &Path, to: &Path) -> bool {
    unsafe { MoveFileW(wide(from).as_ptr(), wide(to).as_ptr()) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_loopback_port_owner_and_protects_sidecars() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_eq!(listeners(port).unwrap(), vec![std::process::id()]);
        assert!(externally_managed(Path::new(
            "L:/CPSwitch/resources/sidecar/windows-x86_64/cliproxyapi.exe"
        )));
        assert!(externally_managed(Path::new(
            "M:/EasyCLIProxyAPI/cpa-core/cli-proxy-api.exe"
        )));
        assert!(!externally_managed(Path::new(
            "M:/CLIProxy/.dev/cli-proxy-api.exe"
        )));
    }
}
