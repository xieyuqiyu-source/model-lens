use super::Management;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;

static PULLING: AtomicBool = AtomicBool::new(false);
struct PullGuard;
impl Drop for PullGuard {
    fn drop(&mut self) {
        PULLING.store(false, Ordering::Release);
    }
}
#[derive(serde::Serialize)]
pub struct PullResult {
    directory: String,
    before: String,
    after: String,
    updated: bool,
}
fn connected_port(app: &tauri::AppHandle) -> Result<u16, String> {
    let state = app.state::<Management>();
    let session = state
        .0
        .try_lock()
        .map_err(|_| "管理操作进行中，请稍后重试")?;
    if !session.snapshot.connected {
        return Err("请先连接 CPA".into());
    }
    Ok(session.snapshot.port)
}
fn management_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/management.html")
}
#[tauri::command]
pub async fn cpa_open_management(app: tauri::AppHandle) -> Result<(), String> {
    let port = connected_port(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        Command::new("/usr/bin/open")
            .arg(management_url(port))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| "无法打开默认浏览器")?
            .success()
            .then_some(())
            .ok_or("CPA 管理页未能打开".into())
    })
    .await
    .map_err(|_| "打开管理页未完成".to_owned())?
}

// Only Git children created here are terminated on timeout; never the running CPA.
fn git(path: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new("/usr/bin/git");
    command
        .current_dir(path)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "submodule.recurse=false",
            "-c",
            "merge.autoStash=false",
            "-c",
            "rebase.autoStash=false",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "/usr/bin/ssh -oBatchMode=yes")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = command
        .spawn()
        .map_err(|_| "无法启动 Git，请检查命令行工具")?;
    let stdout = child.stdout.take().ok_or("无法读取 Git 状态")?;
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
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(50)),
            _ => {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.wait();
                break Err("Git 操作超时或中断，请核实仓库状态后重试");
            }
        }
    };
    let output = reader
        .join()
        .map_err(|_| "Git 输出读取失败")?
        .map_err(|_| "Git 输出读取失败")?;
    if !status?.success() {
        // Raw Git diagnostics can include remote credentials; keep them out of the UI.
        return Err("Git 操作失败：请检查网络、仓库权限和分支状态".into());
    }
    String::from_utf8(output).map_err(|_| "Git 返回无效状态".into())
}
fn local_git(path: &Path, args: &[&str]) -> Result<String, String> {
    git(path, args, Duration::from_secs(5)).map(|s| s.trim().to_owned())
}
fn repository(executable: &Path) -> Result<PathBuf, String> {
    let parent = executable.parent().ok_or("CPA 程序路径无效")?;
    let candidate = if parent.file_name().is_some_and(|s| s == ".dev") {
        parent
            .parent()
            .ok_or("CPA 工作区路径无效")?
            .join("CLIProxyApi")
    } else {
        parent.to_path_buf()
    };
    validate_repository(&candidate)
}
fn validate_repository(path: &Path) -> Result<PathBuf, String> {
    let path = path.canonicalize().map_err(|_| "未找到 CPA 源码仓库")?;
    if !path.join(".git").exists() {
        return Err("当前 CPA 不在可识别的 Git 源码工作区中".into());
    }
    let module = fs::read_to_string(path.join("go.mod")).map_err(|_| "无法核实 CPA 源码类型")?;
    if !module.lines().any(|l| {
        l.trim()
            .starts_with("module github.com/router-for-me/CLIProxyAPI/")
    }) {
        return Err("当前目录不是已支持的 CPA 源码仓库".into());
    }
    let root = local_git(&path, &["rev-parse", "--show-toplevel"])?;
    if Path::new(&root).canonicalize().ok().as_ref() != Some(&path) {
        return Err("CPA 仓库根目录不一致，已停止更新".into());
    }
    Ok(path)
}
fn pull(path: &Path) -> Result<PullResult, String> {
    let path = validate_repository(path)?;
    if !local_git(
        &path,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?
    .is_empty()
    {
        return Err("CPA 仓库有未提交改动，请先处理；未拉取更新".into());
    }
    let branch = local_git(&path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map_err(|_| "CPA 当前未处于分支，请先在仓库切换分支")?;
    let upstream = local_git(&path, &["rev-parse", "--abbrev-ref", "@{upstream}"])
        .map_err(|_| "CPA 当前分支没有跟踪上游，无法拉取")?;
    let remote = local_git(
        &path,
        &["config", "--get", &format!("branch.{branch}.remote")],
    )?;
    let before = local_git(&path, &["rev-parse", "HEAD"])?;
    // Pull's fetch + fast-forward, split so merge can protect ignored local files too.
    git(
        &path,
        &["fetch", "--no-recurse-submodules", "--", &remote],
        Duration::from_secs(120),
    )
    .map_err(|e| format!("{e}；获取更新未完成，请在 CPA 仓库核实"))?;
    if local_git(&path, &["symbolic-ref", "--quiet", "--short", "HEAD"])? != branch
        || local_git(&path, &["rev-parse", "HEAD"])? != before
        || local_git(&path, &["rev-parse", "--abbrev-ref", "@{upstream}"])? != upstream
        || !local_git(
            &path,
            &["status", "--porcelain", "--untracked-files=normal"],
        )?
        .is_empty()
    {
        return Err("拉取期间 CPA 仓库已变化，未应用更新".into());
    }
    let target = local_git(&path, &["rev-parse", "@{upstream}"])?;
    git(
        &path,
        &[
            "merge",
            "--ff-only",
            "--no-overwrite-ignore",
            "--no-autostash",
            "--no-edit",
            &target,
        ],
        Duration::from_secs(30),
    )
    .map_err(|_| {
        "无法安全快进更新：可能存在分支分叉或本地文件冲突；未自动合并，请在 CPA 仓库核实".to_owned()
    })?;
    let after = local_git(&path, &["rev-parse", "HEAD"])?;
    Ok(PullResult {
        directory: path.to_string_lossy().into_owned(),
        updated: before != after,
        before,
        after,
    })
}
#[tauri::command]
pub async fn cpa_pull(app: tauri::AppHandle) -> Result<PullResult, String> {
    let port = connected_port(&app)?;
    if PULLING.swap(true, Ordering::AcqRel) {
        return Err("CPA 更新正在进行".into());
    }
    let guard = PullGuard;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let executable = crate::discovery::connected_executable(port)?;
        let path = repository(&executable)?;
        if connected_port(&app)? != port {
            return Err("CPA 连接已变化，请重试".into());
        }
        pull(&path)
    })
    .await
    .map_err(|_| "CPA 拉取任务未完成".to_owned())?
}
#[cfg(test)]
#[path = "cpa_tests.rs"]
mod tests;
