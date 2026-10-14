mod install;
pub mod jobs;
mod process;
mod runtime;
pub mod versions;
use super::Management;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use tauri::Manager;

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
    process::output(command, timeout)
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
pub async fn cpa_pull(
    app: tauri::AppHandle,
    expected_executable: Option<String>,
) -> Result<String, String> {
    let port = connected_port(&app)?;
    jobs::run(app, move |app| {
        let executable = crate::discovery::connected_executable(port)?;
        if expected_executable
            .as_deref()
            .is_some_and(|expected| Path::new(expected) != executable)
        {
            return Err("所选 CPA 与当前连接不一致，请连接所选服务后再更新".into());
        }
        let layout = runtime::Layout::existing(&executable, port)?;
        let _repository = versions::REPOSITORY_ACCESS
            .lock()
            .map_err(|_| "CPA 仓库状态不可用")?;
        jobs::update_result(app, None);
        jobs::phase(app, "$ git fetch → merge --ff-only · 正在获取更新…");
        let result = pull(&layout.repository)?;
        jobs::phase(app, "$ go build ./cmd/server · 正在编译，原服务继续运行…");
        let staged = runtime::build(&layout)?;
        jobs::phase(app, "$ SIGTERM → CPA --config config.yaml · 正在更新重启…");
        runtime::replace_and_restart(&layout, staged)?;
        jobs::update_result(
            app,
            Some((
                executable.to_string_lossy().into_owned(),
                result.after.clone(),
            )),
        );
        Ok(format!(
            "CPA 更新成功 · {}（已编译并重启）",
            &result.after[..7]
        ))
    })
    .await
}
#[cfg(test)]
#[path = "cpa_tests.rs"]
mod tests;
