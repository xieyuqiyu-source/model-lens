use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
pub(super) fn output(mut command: Command, timeout: Duration) -> Result<String, String> {
    use std::os::unix::process::CommandExt;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|_| "无法启动命令，请检查 Git / Go 命令行工具")?;
    let stdout = child.stdout.take().ok_or("无法读取命令状态")?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(1024 * 1024)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(100)),
            _ => {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.wait();
                break Err("命令超时或中断，请核实状态；未自动重试");
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "命令输出读取失败")?
        .map_err(|_| "命令输出读取失败")?;
    if !status?.success() {
        return Err("命令执行失败，请检查网络、依赖版本或仓库状态".into());
    }
    String::from_utf8(bytes).map_err(|_| "命令返回无效状态".into())
}
