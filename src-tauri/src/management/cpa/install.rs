use super::{
    jobs,
    runtime::{self, Layout},
};
#[cfg(target_os = "macos")]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::Manager;
fn secret() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    #[cfg(target_os = "macos")]
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| "无法生成本地密钥")?;
    #[cfg(windows)]
    crate::windows::cpa::random(&mut bytes).map_err(|_| "无法生成本地密钥")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn destination(value: &str) -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let expanded = if let Some(rest) = value.strip_prefix("~/") {
        std::env::var("HOME").map_err(|_| "无法定位用户目录")? + "/" + rest
    } else {
        value.into()
    };
    #[cfg(windows)]
    let expanded = value.to_owned();
    let path = PathBuf::from(expanded.trim());
    if !path.is_absolute() || path.file_name().is_none() || path.exists() {
        return Err("请选择尚不存在的绝对安装路径，已有目录不会覆盖".into());
    }
    let parent = path.parent().ok_or("安装路径无效")?;
    #[cfg(target_os = "macos")]
    let parent = parent.canonicalize();
    #[cfg(windows)]
    let parent = crate::windows::cpa::canonical(parent);
    let parent = parent.map_err(|_| "安装路径的上级目录不存在")?;
    Ok(parent.join(path.file_name().unwrap()))
}
fn config(port: u16, auth: &Path, key: &str, api_key: &str) -> String {
    format!("config-version: 8\nserver:\n  host: \"127.0.0.1\"\n  port: {port}\nmanagement:\n  allow-remote: false\n  secret-key: {}\n  disable-control-panel: false\n  disable-auto-update-panel: true\naccess:\n  api-keys:\n    - {}\noauth:\n  auth-dir: {}\n",serde_json::to_string(key).unwrap(),serde_json::to_string(api_key).unwrap(),serde_json::to_string(&auth.to_string_lossy()).unwrap())
}
#[derive(serde::Serialize, serde::Deserialize)]
struct PendingConnection {
    port: u16,
    key: String,
}
fn pending_path(layout: &Layout) -> PathBuf {
    layout.directory.join("modellens-pending-connection.json")
}
pub(super) fn restore_connection(app: &tauri::AppHandle, layout: &Layout) -> Result<(), String> {
    #[cfg(windows)]
    if !layout
        .directory
        .file_name()
        .is_some_and(|name| name == ".dev")
    {
        return Ok(());
    }
    let path = pending_path(layout);
    #[cfg(target_os = "macos")]
    let flags = libc::O_NOFOLLOW | libc::O_NONBLOCK;
    #[cfg(windows)]
    let flags = windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(&path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("CPA 已启动，但无法读取本工具生成的安装连接记录".into()),
    };
    if !file.metadata().is_ok_and(|m| {
        #[cfg(windows)]
        if crate::windows::is_reparse_point(&m) {
            return false;
        }
        m.is_file()
    }) {
        return Err("安装连接记录无效".into());
    }
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取安装连接记录")?;
    if bytes.len() > 16384 {
        return Err("安装连接记录无效".into());
    }
    let pending: PendingConnection =
        serde_json::from_slice(&bytes).map_err(|_| "安装连接记录无效")?;
    if pending.port != layout.port {
        return Err("安装连接端口不一致，请使用安装时的端口".into());
    }
    let state = app.state::<super::super::Management>();
    let mut s = state.0.lock().map_err(|_| "管理状态不可用")?;
    let result = super::super::local_connection::LocalFile::for_app(app)
        .and_then(|store| s.connect_and_save(pending.port, pending.key, &store));
    if result.is_err() {
        return Err(format!("CPA 已安装并运行，但自动连接或保存失败。安装目录：{}；连接记录已保留，再次启动/重启可重试",layout.directory.display()));
    }
    fs::remove_file(path).map_err(|_| "连接已保存，但安装临时连接记录清理失败")?;
    Ok(())
}
pub(super) fn install(app: &tauri::AppHandle, value: &str, port: u16) -> Result<String, String> {
    if port == 0 {
        return Err("端口无效".into());
    }
    let data = app.path().app_data_dir().map_err(|_| "无法定位应用目录")?;
    let snapshot = crate::discovery::scan(data.clone())?;
    if !snapshot.status_known {
        return Err("进程检查失败，未开始安装".into());
    }
    #[cfg(target_os = "macos")]
    if snapshot
        .services
        .iter()
        .any(|s| s.pid.is_some() || Path::new(&s.location.directory).exists())
    {
        return Err("已发现 CLIProxy 进程或安装目录，请选择已有服务启动；未重复安装".into());
    }
    let target = destination(value)?;
    runtime::go()?;
    super::git_executable()?;
    std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|_| "端口已被占用，未开始安装")?;
    let staged = tempfile::Builder::new()
        .prefix(".modellens-cpa-install-")
        .tempdir_in(target.parent().unwrap())
        .map_err(|_| "无法写入安装目录")?;
    #[cfg(windows)]
    crate::windows::cpa::private_directory(staged.path())
        .map_err(|_| "无法保护安装目录的 Windows 权限")?;
    jobs::phase(app, "$ git clone CLIProxyAPI · 正在获取官方源码…");
    super::git(
        staged.path(),
        &[
            "clone",
            "--",
            "https://github.com/router-for-me/CLIProxyAPI.git",
            "CLIProxyApi",
        ],
        Duration::from_secs(300),
    )?;
    let directory = staged.path().join(".dev");
    let mut builder = fs::DirBuilder::new();
    #[cfg(target_os = "macos")]
    builder.mode(0o700);
    builder.create(&directory).map_err(|_| "无法创建运行目录")?;
    let layout = Layout {
        repository: staged.path().join("CLIProxyApi"),
        executable: directory.join(if cfg!(windows) {
            "cli-proxy-api.exe"
        } else {
            "cli-proxy-api"
        }),
        config: directory.join("config.yaml"),
        directory: directory.clone(),
        port,
    };
    super::validate_repository(&layout.repository)?;
    jobs::phase(app, "$ go build ./cmd/server · 正在编译…");
    let binary = runtime::build(&layout)?;
    fs::rename(&binary, &layout.executable).map_err(|_| "无法保存 CPA 程序")?;
    let key = secret()?;
    let api_key = secret()?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(target_os = "macos")]
    options.mode(0o600);
    let mut file = options
        .open(&layout.config)
        .map_err(|_| "无法初始化本地配置")?;
    file.write_all(config(port, &target.join(".dev/auths"), &key, &api_key).as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| "无法保存本地配置")?;
    let pending = PendingConnection { port, key };
    let mut recovery = options
        .open(pending_path(&layout))
        .map_err(|_| "无法保护安装连接记录")?;
    recovery
        .write_all(&serde_json::to_vec(&pending).map_err(|_| "无法保存安装连接记录")?)
        .and_then(|_| recovery.sync_all())
        .map_err(|_| "无法保存安装连接记录")?;
    drop((file, recovery));
    #[cfg(target_os = "macos")]
    fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o700))
        .map_err(|_| "无法保护安装目录")?;
    #[cfg(target_os = "macos")]
    let executable = target
        .join(".dev")
        .join(layout.executable.file_name().ok_or("安装路径无效")?);
    #[cfg(windows)]
    let executable = target.join(".dev/cli-proxy-api.exe");
    #[cfg(target_os = "macos")]
    crate::discovery::remember_install(&data, &executable)?;
    if !promote(staged.path(), &target)? {
        return Err("安装路径已存在或无法写入；未覆盖任何内容".into());
    }
    let _ = staged.keep();
    #[cfg(windows)]
    crate::discovery::remember_install(&data, &executable)
        .map_err(|error| format!("{error}；安装文件保留在 {}", target.display()))?;
    let layout = Layout::existing(&executable, port)?;
    jobs::phase(app, "$ CPA --config config.yaml · 正在启动并连接…");
    runtime::launch(&layout).map_err(|e| format!("{e}；安装文件保留在 {}", target.display()))?;
    let _ = crate::discovery::scan(data);
    restore_connection(app, &layout)?;
    Ok("CLIProxy 已安装、启动并保存本机管理连接".into())
}
fn promote(from: &Path, to: &Path) -> Result<bool, String> {
    #[cfg(windows)]
    return Ok(crate::windows::cpa::promote(from, to));
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        let from = CString::new(from.as_os_str().as_encoded_bytes()).map_err(|_| "安装路径无效")?;
        let to = CString::new(to.as_os_str().as_encoded_bytes()).map_err(|_| "安装路径无效")?;
        Ok(unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } == 0)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn install_paths_never_replace_existing_directories_and_config_is_local() {
        let temp = tempfile::tempdir().unwrap();
        assert!(destination(temp.path().to_str().unwrap()).is_err());
        assert!(destination("relative/path").is_err());
        #[cfg(target_os = "macos")]
        let canonical = temp.path().canonicalize().unwrap();
        #[cfg(windows)]
        let canonical = crate::windows::cpa::canonical(temp.path()).unwrap();
        assert_eq!(
            destination(temp.path().join("new install").to_str().unwrap()).unwrap(),
            canonical.join("new install")
        );
        let c = config(
            8317,
            Path::new("/synthetic/auths"),
            "synthetic-management",
            "synthetic-client",
        );
        assert!(c.contains("127.0.0.1"));
        assert!(c.contains("allow-remote: false"));
        assert!(!c.contains("your-api-key"));
        assert_eq!(secret().unwrap().len(), 64);
        assert_ne!(secret().unwrap(), secret().unwrap());
    }
}
