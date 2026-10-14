use super::{process, repository};
use std::os::unix::{
    fs::{OpenOptionsExt, PermissionsExt},
    process::CommandExt,
};
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
pub(super) struct Layout {
    pub repository: PathBuf,
    pub directory: PathBuf,
    pub executable: PathBuf,
    pub config: PathBuf,
    pub port: u16,
}
impl Layout {
    pub fn existing(executable: &Path, port: u16) -> Result<Self, String> {
        if port == 0 {
            return Err("端口无效".into());
        }
        let repository = repository(executable)?;
        let executable = executable
            .canonicalize()
            .map_err(|_| "CPA 程序不存在，请先安装")?;
        let directory = executable.parent().ok_or("CPA 路径无效")?.to_path_buf();
        let config = directory.join("config.yaml");
        if !fs::symlink_metadata(&config).is_ok_and(|m| m.is_file()) {
            return Err("未找到同目录 config.yaml，无法安全启动".into());
        }
        Ok(Self {
            repository,
            directory,
            executable,
            config,
            port,
        })
    }
}
pub(super) fn go() -> Result<PathBuf, String> {
    for path in [
        "/opt/homebrew/bin/go",
        "/usr/local/go/bin/go",
        "/usr/local/bin/go",
    ] {
        if Path::new(path).is_file() {
            return Ok(path.into());
        }
    }
    Err("未找到 Go，请先安装 Go 1.26 或更高版本".into())
}
pub(super) fn build(layout: &Layout) -> Result<tempfile::NamedTempFile, String> {
    build_with(layout, &go()?)
}
fn build_with(layout: &Layout, tool: &Path) -> Result<tempfile::NamedTempFile, String> {
    let staged = tempfile::Builder::new()
        .prefix(".cpa-build-")
        .tempfile_in(&layout.directory)
        .map_err(|_| "无法创建编译临时文件")?;
    let mut cmd = Command::new(tool);
    cmd.current_dir(&layout.repository)
        .args(["build", "-o"])
        .arg(staged.path())
        .arg("./cmd/server")
        .env(
            "PATH",
            "/opt/homebrew/bin:/usr/local/go/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        );
    process::output(cmd, Duration::from_secs(900)).map_err(|_| {
        "CPA 编译失败或超时；未停止原服务，请检查 Go 版本、依赖网络和源码".to_owned()
    })?;
    fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o700))
        .map_err(|_| "无法设置 CPA 可执行权限")?;
    Ok(staged)
}
fn listeners(port: u16) -> Vec<u32> {
    crate::discovery::command_output(
        "/usr/sbin/lsof",
        &["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fp"],
    )
    .unwrap_or_default()
    .lines()
    .filter_map(|s| s.strip_prefix('p')?.parse().ok())
    .collect()
}
fn owner(layout: &Layout) -> Result<Option<u32>, String> {
    if TcpListener::bind(("127.0.0.1", layout.port)).is_ok() {
        return Ok(None);
    }
    let pids = listeners(layout.port);
    if pids.len() != 1 {
        return Err("端口被占用，无法确认 CPA 进程；未停止任何服务".into());
    }
    let pid = pids[0];
    let output =
        crate::discovery::command_output("/bin/ps", &["-p", &pid.to_string(), "-o", "pid=,comm="])?;
    let (_, exe) = output
        .lines()
        .find_map(crate::discovery::parse_process)
        .ok_or("占用端口的不是可识别 CPA，已停止操作")?;
    if Path::new(&exe).canonicalize().ok().as_ref() != Some(&layout.executable) {
        return Err("端口属于其他程序，已停止操作".into());
    }
    Ok(Some(pid))
}
fn stop(layout: &Layout) -> Result<(), String> {
    let Some(pid) = owner(layout)? else {
        return Ok(());
    };
    let parent =
        crate::discovery::command_output("/bin/ps", &["-p", &pid.to_string(), "-o", "ppid="])?
            .trim()
            .parse::<u32>()
            .map_err(|_| "无法核实 CPA 启动器")?;
    let lock = layout.directory.join("dev.lock");
    let holders = crate::discovery::command_output(
        "/usr/sbin/lsof",
        &["-t", lock.to_str().ok_or("路径无效")?],
    )
    .unwrap_or_default();
    let supervised = parent > 1 && holders.lines().any(|s| s == parent.to_string());
    if owner(layout)? != Some(pid) {
        return Err("CPA 进程已变化，未发送停止信号".into());
    }
    // User authorized taking over this dev launcher, including its CPM child.
    let target = if supervised { parent } else { pid };
    if unsafe { libc::kill(target as i32, libc::SIGTERM) } != 0 {
        return Err("无法正常停止 CPA / 原启动器".into());
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let cpa_gone = !listeners(layout.port).contains(&pid);
        let launcher_done = !supervised
            || crate::discovery::command_output("/usr/sbin/lsof", &["-t", lock.to_str().unwrap()])
                .unwrap_or_default()
                .lines()
                .all(|s| s != parent.to_string());
        if cpa_gone && launcher_done && TcpListener::bind(("127.0.0.1", layout.port)).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err("CPA 未正常停止或端口被重新占用；未强制终止其他进程".into())
}
fn terminate_child(child: &mut Child) {
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
}
pub(super) fn launch(layout: &Layout) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(1))
        .build()
        .map_err(|_| "无法检查 CPA 启动状态")?;
    let _port = TcpListener::bind(("127.0.0.1", layout.port))
        .map_err(|_| "端口被占用，未启动第二个 CPA")?;
    let log = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(layout.directory.join("cpa-modellens.log"))
        .map_err(|_| "无法创建 CPA 本地日志")?;
    log.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| "无法保护 CPA 日志")?;
    let mut cmd = Command::new(&layout.executable);
    cmd.current_dir(&layout.directory)
        .arg("--config")
        .arg(&layout.config)
        .env(
            "LKS_TZ_STATE_FILE",
            layout
                .directory
                .join("plugins/timezone-override/state.json"),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone().map_err(|_| "无法打开 CPA 日志")?,
        ))
        .stderr(Stdio::from(log))
        .process_group(0);
    drop(_port);
    let mut child = cmd.spawn().map_err(|_| "CPA 启动失败")?;
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return Err("CPA 提前退出，请查看安装目录 .dev/cpa-modellens.log".into()),
            Err(_) => {
                terminate_child(&mut child);
                return Err("CPA 进程状态不可用，已停止本次新进程".into());
            }
            Ok(None) => {}
        }
        if client
            .get(format!("http://127.0.0.1:{}/", layout.port))
            .send()
            .is_ok_and(|r| r.status().is_success())
            && listeners(layout.port).contains(&child.id())
        {
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    terminate_child(&mut child);
    Err("CPA 启动检查超时，已停止本次新进程".into())
}
pub(super) fn restart(layout: &Layout) -> Result<(), String> {
    stop(layout)?;
    launch(layout)
}
pub(super) fn replace_and_restart(
    layout: &Layout,
    staged: tempfile::NamedTempFile,
) -> Result<(), String> {
    let backup = tempfile::Builder::new()
        .prefix(".cpa-backup-")
        .tempdir_in(&layout.directory)
        .map_err(|_| "无法备份原 CPA")?;
    fs::copy(&layout.executable, backup.path().join("cpa"))
        .map_err(|_| "备份原 CPA 失败，未停止服务")?;
    fs::copy(&layout.config, backup.path().join("config.yaml"))
        .map_err(|_| "备份配置失败，未停止服务")?;
    fs::set_permissions(
        backup.path().join("config.yaml"),
        fs::Permissions::from_mode(0o600),
    )
    .map_err(|_| "无法保护配置备份")?;
    if let Err(error) = stop(layout) {
        let recovered = owner(layout).ok().flatten().is_some() || launch(layout).is_ok();
        let path = backup.keep();
        return Err(format!(
            "{error}；{}；备份保留在 {}",
            if recovered {
                "原程序仍在运行或已重新启动"
            } else {
                "原服务未恢复，请核实端口"
            },
            path.display()
        ));
    }
    if fs::rename(staged.path(), &layout.executable).is_err() {
        if launch(layout).is_ok() {
            return Err("替换 CPA 失败，原程序已重新启动".into());
        }
        let path = backup.keep();
        return Err(format!(
            "替换 CPA 失败，原服务未恢复；备份保留在 {}",
            path.display()
        ));
    }
    if let Err(error) = launch(layout) {
        let restored = fs::copy(backup.path().join("cpa"), &layout.executable)
            .and_then(|_| fs::copy(backup.path().join("config.yaml"), &layout.config));
        if restored.is_ok() && launch(layout).is_ok() {
            return Err(format!("{error}；已恢复原程序与配置并重新启动"));
        }
        let path = backup.keep();
        return Err(format!(
            "{error}；恢复未完成，原程序和配置保留在 {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
