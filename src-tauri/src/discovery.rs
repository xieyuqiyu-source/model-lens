use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Remembered {
    pub executable: String,
    pub directory: String,
    pub working_directory: Option<String>,
    pub last_seen: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    #[serde(flatten)]
    pub location: Remembered,
    pub pid: Option<u32>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub services: Vec<Service>,
    pub checked_at: u64,
    pub status_known: bool,
    pub warning: Option<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// Only executable names are inspected: command arguments and environment can contain secrets.
pub fn parse_process(line: &str) -> Option<(u32, String)> {
    let line = line.trim();
    let split = line.find(char::is_whitespace)?;
    let pid = line[..split].parse().ok()?;
    let path = line[split..].trim();
    let p = Path::new(path);
    if !p.is_absolute() {
        return None;
    }
    let name = p.file_name()?.to_str()?.to_ascii_lowercase();
    let known = matches!(
        name.as_str(),
        "cliproxyapi" | "cli-proxy-api" | "cliproxy" | "cliproxyapi-plus" | "cli-proxy-api-plus"
    );
    let dev_cpa = name == "cpa"
        && p.ancestors().skip(1).any(|a| {
            a.file_name()
                .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("cliproxy"))
        });
    (known || dev_cpa).then(|| (pid, path.to_owned()))
}
pub(crate) fn command_output(program: &str, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "无法运行系统进程检查工具".to_owned())?;
    let stdout = child.stdout.take().ok_or("无法读取进程检查结果")?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(2 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < Duration::from_secs(3) => {
                thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("进程检查超时，请重试".into());
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "进程检查失败")?
        .map_err(|_| "无法读取进程检查结果")?;
    if !status.success() {
        return Err("系统未返回可访问的进程目录".into());
    }
    String::from_utf8(bytes).map_err(|_| "系统返回了无法识别的路径".into())
}
pub(crate) fn working_directory(pid: u32) -> Option<String> {
    let output = command_output(
        "/usr/sbin/lsof",
        &["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"],
    )
    .ok()?;
    output.lines().find_map(|s| {
        s.strip_prefix('n')
            .filter(|s| Path::new(s).is_absolute())
            .map(str::to_owned)
    })
}
pub fn connected_executable(port: u16) -> Result<PathBuf, String> {
    let listeners = command_output(
        "/usr/sbin/lsof",
        &["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fp"],
    )
    .map_err(|_| "未找到当前端口的 CPA 进程")?;
    let uid = unsafe { libc::geteuid() }.to_string();
    let processes = command_output("/bin/ps", &["-U", &uid, "-o", "pid=,comm="])?;
    let matches: Vec<_> = processes
        .lines()
        .filter_map(parse_process)
        .filter(|(pid, _)| listeners.lines().any(|line| line == format!("p{pid}")))
        .collect();
    if matches.len() != 1 {
        return Err("无法唯一确认当前连接对应的 CPA 进程".into());
    }
    Ok(PathBuf::from(&matches[0].1))
}
fn project_directory(executable: &str) -> String {
    let parent = Path::new(executable).parent().unwrap_or(Path::new("/"));
    // DevDock development builds commonly live in a project's .dev directory.
    let directory = if parent.file_name().is_some_and(|s| s == ".dev") {
        parent.parent().unwrap_or(parent)
    } else {
        parent
    };
    directory.to_string_lossy().into_owned()
}
fn read_cache(path: &Path) -> Result<Vec<Remembered>, String> {
    match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "已记住的位置文件损坏，原文件已保留".into())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err("无法读取已记住的位置，请检查数据目录权限".into()),
    }
}
fn save_cache(path: &Path, records: &[Remembered]) -> Result<(), String> {
    let parent = path.parent().ok_or("数据目录无效")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建本地数据目录")?;
    let temp = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(records).map_err(|_| "无法保存服务位置")?;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|_| "无法保护本地数据目录")?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|_| "无法保存服务位置")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "写入服务位置失败")?;
    }
    #[cfg(not(unix))]
    fs::write(&temp, bytes).map_err(|_| "无法保存服务位置")?;
    fs::rename(temp, path).map_err(|_| "无法更新已记住的位置".into())
}
fn merge(remembered: Vec<Remembered>, live: &[(u32, Remembered)]) -> Vec<Service> {
    let mut services: Vec<Service> = live
        .iter()
        .map(|(pid, r)| Service {
            location: r.clone(),
            pid: Some(*pid),
        })
        .collect();
    for r in remembered {
        if !live.iter().any(|(_, l)| l.executable == r.executable) {
            services.push(Service {
                location: r,
                pid: None,
            });
        }
    }
    services
}
pub(crate) fn remember_install(data: &Path, executable: &Path) -> Result<(), String> {
    let path = data.join("discovery.json");
    let mut records = read_cache(&path)?;
    let executable = executable.to_string_lossy().into_owned();
    if !records.iter().any(|r| r.executable == executable) {
        records.push(Remembered {
            directory: project_directory(&executable),
            working_directory: Path::new(&executable)
                .parent()
                .map(|p| p.to_string_lossy().into_owned()),
            executable,
            last_seen: now(),
        });
    }
    save_cache(&path, &records)
}
pub fn scan(data_dir: PathBuf) -> Result<Snapshot, String> {
    if !cfg!(target_os = "macos") {
        return Err("第一阶段仅支持 macOS 的进程发现".into());
    }
    let cache_path = data_dir.join("discovery.json");
    let cache = read_cache(&cache_path);
    let can_save = cache.is_ok();
    let mut warning = cache.as_ref().err().cloned();
    let remembered = cache.unwrap_or_default();
    #[cfg(unix)]
    let uid = unsafe { libc::geteuid() }.to_string();
    #[cfg(not(unix))]
    let uid = String::new();
    let checked_at = now();
    let output = match command_output("/bin/ps", &["-U", &uid, "-o", "pid=,comm="]) {
        Ok(output) => output,
        Err(error) => {
            return Ok(Snapshot {
                services: merge(remembered, &[]),
                checked_at,
                warning: Some(error),
                status_known: false,
            })
        }
    };
    let mut live = Vec::new();
    for (pid, executable) in output.lines().filter_map(parse_process) {
        if !Path::new(&executable).is_file() {
            continue;
        }
        let cwd = working_directory(pid);
        if cwd.is_none() {
            warning = Some("部分进程的工作目录不可访问；程序路径已识别".into());
        }
        let record = Remembered {
            directory: project_directory(&executable),
            executable,
            working_directory: cwd,
            last_seen: checked_at,
        };
        live.push((pid, record));
    }
    let services = merge(remembered, &live);
    if can_save && !live.is_empty() {
        let mut records = Vec::new();
        for s in &services {
            if !records
                .iter()
                .any(|r: &Remembered| r.executable == s.location.executable)
            {
                records.push(s.location.clone());
            }
        }
        if let Err(error) = save_cache(&cache_path, &records) {
            warning = Some(error);
        }
    }
    Ok(Snapshot {
        services,
        checked_at,
        status_known: true,
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(path: &str) -> Remembered {
        Remembered {
            executable: path.into(),
            directory: "/tmp".into(),
            working_directory: None,
            last_seen: 1,
        }
    }
    #[test]
    fn installation_is_remembered_before_first_start_and_preserves_existing_locations() {
        let temp = tempfile::tempdir().unwrap();
        remember_install(
            temp.path(),
            Path::new("/synthetic/first/.dev/cli-proxy-api"),
        )
        .unwrap();
        remember_install(
            temp.path(),
            Path::new("/synthetic/second/.dev/cli-proxy-api"),
        )
        .unwrap();
        remember_install(
            temp.path(),
            Path::new("/synthetic/second/.dev/cli-proxy-api"),
        )
        .unwrap();
        let records = read_cache(&temp.path().join("discovery.json")).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].directory, "/synthetic/second");
        assert!(merge(records, &[]).iter().all(|s| s.pid.is_none()));
        fs::write(temp.path().join("discovery.json"), b"broken").unwrap();
        assert!(
            remember_install(temp.path(), Path::new("/synthetic/new/.dev/cli-proxy-api")).is_err()
        );
        assert_eq!(
            fs::read(temp.path().join("discovery.json")).unwrap(),
            b"broken"
        );
    }
    #[test]
    fn recognizes_only_supported_executables() {
        assert_eq!(
            parse_process(" 123 /tmp/My Project/CLIProxy/.dev/cpa"),
            Some((123, "/tmp/My Project/CLIProxy/.dev/cpa".into()))
        );
        assert!(parse_process("123 /tmp/cliproxyapi").is_some());
        for unrelated in [
            "123 /tmp/cpa",
            "123 /tmp/cliproxyapi-helper",
            "123 /tmp/CLIProxy/node",
            "bad /tmp/cliproxyapi",
            "123 cliproxyapi",
        ] {
            assert!(parse_process(unrelated).is_none());
        }
    }
    #[test]
    fn missing_process_stays_remembered_not_running() {
        let old = record("/tmp/cliproxyapi");
        let merged = merge(vec![old.clone()], &[]);
        assert_eq!(merged[0].pid, None);
        assert_eq!(merged[0].location, old);
        assert_eq!(merge(vec![old.clone()], &[(99, old)]).len(), 1);
    }
    #[test]
    fn cache_round_trip_and_corruption() {
        let dir =
            std::env::temp_dir().join(format!("modellens-test-{}-{}", std::process::id(), now()));
        let path = dir.join("discovery.json");
        assert!(read_cache(&path).unwrap().is_empty());
        let records = vec![record("/tmp/项目 with spaces/cliproxyapi")];
        save_cache(&path, &records).unwrap();
        assert_eq!(read_cache(&path).unwrap(), records);
        fs::write(&path, b"broken").unwrap();
        assert!(read_cache(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn dev_directory_is_not_confused_with_project() {
        assert_eq!(project_directory("/tmp/CLIProxy/.dev/cpa"), "/tmp/CLIProxy");
        assert_eq!(project_directory("/opt/bin/cliproxyapi"), "/opt/bin");
    }
    #[test]
    fn failed_system_command_is_not_an_empty_success() {
        assert!(command_output("/usr/bin/false", &[]).is_err());
    }
}
