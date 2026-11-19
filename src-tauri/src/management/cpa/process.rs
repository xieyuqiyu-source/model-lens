use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
pub(super) fn output(mut command: Command, timeout: Duration) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| "无法启动命令，请检查 Git / Go 命令行工具")?;
    #[cfg(windows)]
    let job = match crate::windows::cpa::attach_job(&child) {
        Ok(job) => Some(job),
        Err(_) if child.try_wait().ok().flatten().is_some() => None,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("无法跟踪 Git / Go 进程，已停止该命令".into());
        }
    };
    let stdout = child.stdout.take().ok_or("无法读取命令状态")?;
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(1024 * 1024)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = send.send(result);
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(100)),
            _ => {
                #[cfg(target_os = "macos")]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                #[cfg(windows)]
                let _ = child.kill();
                let _ = child.wait();
                break Err("命令超时或中断，请核实状态；未自动重试");
            }
        }
    };
    #[cfg(windows)]
    drop(job);
    let status = status?;
    let bytes = receive
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| "命令输出读取超时")?
        .map_err(|_| "命令输出读取失败")?;
    if !status.success() {
        return Err("命令执行失败，请检查网络、依赖版本或仓库状态".into());
    }
    String::from_utf8(bytes).map_err(|_| "命令返回无效状态".into())
}
