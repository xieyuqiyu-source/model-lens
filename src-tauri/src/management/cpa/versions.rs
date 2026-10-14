use super::{git, local_git, process, repository, runtime};
use std::{
    path::Path,
    process::Command,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

pub(super) static REPOSITORY_ACCESS: Mutex<()> = Mutex::new(());
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Versions {
    source: String,
    local: Option<String>,
    remote: Option<String>,
    branch: String,
    upstream: Option<String>,
    ahead: u32,
    behind: u32,
    dirty: bool,
    status: &'static str,
    remote_error: Option<String>,
    checked_at: u64,
}
fn build_info(output: &str) -> (Option<String>, Option<bool>) {
    let revision = output
        .lines()
        .find_map(|l| l.trim().strip_prefix("build\tvcs.revision="))
        .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_owned);
    let dirty = output
        .lines()
        .find_map(|l| l.trim().strip_prefix("build\tvcs.modified="))
        .and_then(|s| match s {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        });
    (revision, dirty)
}
fn binary_version(executable: &Path) -> (Option<String>, Option<bool>) {
    let Ok(go) = runtime::go() else {
        return (None, None);
    };
    let mut command = Command::new(go);
    command.args(["version", "-m"]).arg(executable);
    process::output(command, Duration::from_secs(10))
        .map(|s| build_info(&s))
        .unwrap_or_default()
}
fn classify(v: &Versions, build_dirty: Option<bool>) -> &'static str {
    if v.remote_error.is_some() || v.remote.is_none() {
        return "unknown";
    }
    if v.ahead > 0 && v.behind > 0 {
        return "diverged";
    }
    if v.behind > 0 {
        return "behind";
    }
    if v.ahead > 0 {
        return "ahead";
    }
    if v.dirty || build_dirty == Some(true) {
        return "modified";
    }
    match (&v.local, build_dirty) {
        (Some(local), Some(false)) if Some(local) == v.remote.as_ref() => "current",
        (Some(_), Some(false)) => "unbuilt",
        _ => "unknown",
    }
}
fn inspect(path: &Path, build: (Option<String>, Option<bool>)) -> Result<Versions, String> {
    let source = local_git(path, &["rev-parse", "HEAD"])?;
    let branch = local_git(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .unwrap_or_else(|_| "游离 HEAD".into());
    let upstream = local_git(path, &["rev-parse", "--abbrev-ref", "@{upstream}"]).ok();
    let mut version = Versions {
        source,
        local: build.0,
        remote: local_git(path, &["rev-parse", "@{upstream}"]).ok(),
        branch: branch.clone(),
        upstream: upstream.clone(),
        ahead: 0,
        behind: 0,
        dirty: !local_git(path, &["status", "--porcelain", "--untracked-files=normal"])?.is_empty(),
        status: "unknown",
        remote_error: None,
        checked_at: 0,
    };
    let refresh = || -> Result<(String, u32, u32), String> {
        if upstream.is_none() {
            return Err("未设置跟踪分支，无法检查远端".into());
        }
        let remote = local_git(
            path,
            &["config", "--get", &format!("branch.{branch}.remote")],
        )?;
        let merge_key = format!("branch.{branch}.merge");
        let merge = local_git(path, &["config", "--get", &merge_key])?;
        let tracking = local_git(path, &["rev-parse", "--symbolic-full-name", "@{upstream}"])?;
        if remote == "."
            || !merge.starts_with("refs/heads/")
            || !tracking.starts_with("refs/remotes/")
        {
            return Err("未设置远端跟踪分支，无法检查远端".into());
        }
        // Fetch the tracked branch explicitly: a deleted remote branch must not look current.
        let refspec = format!("{merge}:{tracking}");
        git(
            path,
            &[
                "fetch",
                "--no-tags",
                "--no-recurse-submodules",
                "--",
                &remote,
                &refspec,
            ],
            Duration::from_secs(30),
        )
        .map_err(|_| "远端检查失败，当前显示上次已知版本".to_owned())?;
        if local_git(path, &["rev-parse", "HEAD"])? != version.source
            || local_git(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])? != branch
            || local_git(path, &["rev-parse", "--abbrev-ref", "@{upstream}"]).ok() != upstream
            || local_git(path, &["config", "--get", &merge_key])? != merge
            || local_git(
                path,
                &["config", "--get", &format!("branch.{branch}.remote")],
            )? != remote
        {
            return Err("检查期间分支或源码发生变化，请重新检测".into());
        }
        let remote_sha = local_git(path, &["rev-parse", "@{upstream}"])?;
        let counts = local_git(
            path,
            &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        )?;
        let mut counts = counts
            .split_whitespace()
            .filter_map(|n| n.parse::<u32>().ok());
        Ok((
            remote_sha,
            counts.next().ok_or("无法比较版本")?,
            counts.next().ok_or("无法比较版本")?,
        ))
    };
    match refresh() {
        Ok((remote, ahead, behind)) => {
            version.remote = Some(remote);
            version.ahead = ahead;
            version.behind = behind;
            version.checked_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            version.dirty =
                !local_git(path, &["status", "--porcelain", "--untracked-files=normal"])?
                    .is_empty();
        }
        Err(error) => version.remote_error = Some(error),
    }
    version.status = classify(&version, build.1);
    Ok(version)
}
#[tauri::command]
pub async fn cpa_versions(app: tauri::AppHandle, executable: String) -> Result<Versions, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = REPOSITORY_ACCESS
            .try_lock()
            .map_err(|_| "版本检查或更新正在进行，请稍后重试")?;
        let snapshot =
            crate::discovery::scan(app.path().app_data_dir().map_err(|_| "无法定位应用目录")?)?;
        if !snapshot
            .services
            .iter()
            .any(|s| s.location.executable == executable)
        {
            return Err("未找到已发现的 CPA，请重新检测".into());
        }
        let executable = Path::new(&executable);
        let path = repository(executable)?;
        inspect(&path, binary_version(executable))
    })
    .await
    .map_err(|_| "版本检查未完成".to_owned())?
}

#[cfg(test)]
#[path = "versions_tests.rs"]
mod tests;
