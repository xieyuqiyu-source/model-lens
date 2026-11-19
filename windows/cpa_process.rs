use super::{canonical, externally_managed, wide};
use std::{
    ffi::OsString,
    fs::File,
    io,
    mem::{size_of, size_of_val},
    os::windows::{
        ffi::OsStringExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    },
    path::{Path, PathBuf},
    process::Child,
    ptr::{null, null_mut},
    sync::Mutex,
};
use windows_sys::Win32::{
    Foundation::{
        DuplicateHandle, GetLastError, DUPLICATE_SAME_ACCESS, ERROR_NO_MORE_FILES,
        INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    System::{
        Console::{
            AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, GetConsoleProcessList,
            SetConsoleCtrlHandler, CTRL_BREAK_EVENT,
        },
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{
            CreateProcessW, DeleteProcThreadAttributeList, GetCurrentProcess, GetCurrentProcessId,
            InitializeProcThreadAttributeList, OpenProcess, QueryFullProcessImageNameW,
            TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NEW_CONSOLE,
            EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESHOWWINDOW,
            STARTF_USESTDHANDLES, STARTUPINFOEXW,
        },
    },
    UI::WindowsAndMessaging::SW_HIDE,
};

pub(super) fn handle(raw: RawHandle) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

pub(crate) struct CpaProcess {
    process: OwnedHandle,
    pub(crate) pid: u32,
}
impl CpaProcess {
    fn open(pid: u32) -> io::Result<Self> {
        let process = handle(unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        })?;
        Ok(Self { process, pid })
    }

    pub(crate) fn exited(&self, timeout_ms: u32) -> io::Result<bool> {
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), timeout_ms) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }

    pub(crate) fn terminate_own_child(&self) -> bool {
        unsafe { TerminateProcess(self.process.as_raw_handle(), 1) };
        self.exited(3000).unwrap_or(false)
    }

    fn executable(&self) -> io::Result<PathBuf> {
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        if unsafe {
            QueryFullProcessImageNameW(self.process.as_raw_handle(), 0, buf.as_mut_ptr(), &mut len)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        canonical(&PathBuf::from(OsString::from_wide(&buf[..len as usize])))
    }
}

pub(crate) fn executable(pid: u32) -> Option<PathBuf> {
    CpaProcess::open(pid).ok()?.executable().ok()
}

pub(crate) fn processes() -> Result<Vec<(u32, String)>, String> {
    let error = "无法读取 Windows 进程列表";
    let snapshot =
        handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }).map_err(|_| error)?;
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut success = unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) };
    let mut result = Vec::new();
    while success != 0 {
        let len = entry
            .szExeFile
            .iter()
            .position(|&ch| ch == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "cliproxyapi.exe"
                | "cli-proxy-api.exe"
                | "cliproxy.exe"
                | "cliproxyapi-plus.exe"
                | "cli-proxy-api-plus.exe"
                | "cpa.exe"
        ) {
            if let Some(path) = executable(entry.th32ProcessID) {
                result.push((entry.th32ProcessID, path.to_string_lossy().into_owned()));
            }
        }
        success = unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) };
    }
    if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
        return Err(error.into());
    }
    Ok(result)
}

fn inheritable(file: &File) -> io::Result<OwnedHandle> {
    let mut raw = null_mut();
    let current = unsafe { GetCurrentProcess() };
    if unsafe {
        DuplicateHandle(
            current,
            file.as_raw_handle(),
            current,
            &mut raw,
            0,
            1,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    handle(raw)
}

pub(crate) fn spawn(
    executable: &Path,
    config: &Path,
    directory: &Path,
    log: &File,
) -> io::Result<CpaProcess> {
    let stdin = inheritable(&File::open("NUL")?)?;
    let stdout = inheritable(log)?;
    let allowed = [stdin.as_raw_handle(), stdout.as_raw_handle()];
    let mut size = 0;
    unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size) };
    let mut attributes = vec![0usize; size.div_ceil(size_of::<usize>())];
    let list = attributes.as_mut_ptr().cast();
    if unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES;
    startup.StartupInfo.wShowWindow = SW_HIDE as u16;
    startup.StartupInfo.hStdInput = allowed[0];
    startup.StartupInfo.hStdOutput = allowed[1];
    startup.StartupInfo.hStdError = allowed[1];
    startup.lpAttributeList = list;
    let mut command = OsString::from("\"");
    command.push(executable);
    command.push("\" --config \"");
    command.push(config);
    command.push("\"");
    let mut command = wide(command);
    let mut info = PROCESS_INFORMATION::default();
    let launched = unsafe {
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            allowed.as_ptr().cast(),
            size_of_val(&allowed),
            null_mut(),
            null(),
        ) != 0
            && CreateProcessW(
                wide(executable).as_ptr(),
                command.as_mut_ptr(),
                null(),
                null(),
                1,
                CREATE_NEW_CONSOLE | EXTENDED_STARTUPINFO_PRESENT,
                null(),
                wide(directory).as_ptr(),
                &startup.StartupInfo,
                &mut info,
            ) != 0
    };
    let result = if launched {
        drop(handle(info.hThread));
        handle(info.hProcess).map(|process| CpaProcess {
            process,
            pid: info.dwProcessId,
        })
    } else {
        Err(io::Error::last_os_error())
    };
    unsafe { DeleteProcThreadAttributeList(list) };
    result
}

static CONSOLE: Mutex<()> = Mutex::new(());
unsafe extern "system" fn ignore_break(event: u32) -> i32 {
    i32::from(event == CTRL_BREAK_EVENT)
}

pub(crate) fn stop(pid: u32, expected_path: &Path, port: u16) -> Result<(), String> {
    let error = "无法安全关闭此 CPA，请从其原启动入口关闭";
    let process = CpaProcess::open(pid).map_err(|_| error)?;
    if externally_managed(expected_path)
        || process.executable().ok().as_deref() != Some(expected_path)
        || super::listeners(port)? != [pid]
    {
        return Err(error.into());
    }
    let _guard = CONSOLE.lock().map_err(|_| error)?;
    if unsafe { AttachConsole(pid) } == 0 {
        return Err(error.into());
    }
    let mut attached = [0; 3];
    let count = unsafe { GetConsoleProcessList(attached.as_mut_ptr(), attached.len() as u32) };
    let self_pid = unsafe { GetCurrentProcessId() };
    // A console-wide Ctrl+Break is safe only when it contains the verified CPA and this app.
    let exclusive = count == 2 && attached[..2].contains(&pid) && attached[..2].contains(&self_pid);
    let sent = exclusive
        && process.exited(0).is_ok_and(|exited| !exited)
        && super::listeners(port).is_ok_and(|pids| pids == [pid])
        && unsafe {
            SetConsoleCtrlHandler(Some(ignore_break), 1) != 0
                && GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, 0) != 0
        };
    let exited = sent && process.exited(15000).unwrap_or(false);
    unsafe { FreeConsole() };
    if !sent {
        return Err(error.into());
    }
    if !exited || !super::listeners(port)?.is_empty() {
        return Err("CPA 未正常退出或端口被重新占用".into());
    }
    Ok(())
}

pub(crate) fn attach_job(child: &Child) -> io::Result<OwnedHandle> {
    let job = handle(unsafe { CreateJobObjectW(null(), null()) })?;
    let mut limit = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limit.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&limit as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of_val(&limit) as u32,
        ) == 0
            || AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) == 0
    } {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}
