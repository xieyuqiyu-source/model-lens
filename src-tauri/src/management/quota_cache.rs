use super::{quota::QuotaItem, Credential, Record, Session};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::Path,
};

const LIMIT: u64 = 4 * 1024 * 1024;
const READ_ERROR: &str = "本地额度缓存无法读取，请手动刷新额度";

// Store only displayed quota data; never serialize credentials or transport configuration.
#[derive(Serialize, Deserialize)]
pub(super) struct QuotaData {
    five: Option<f64>,
    week: Option<f64>,
    reset_five: String,
    reset_week: String,
    quota_items: Vec<QuotaItem>,
    plan: Option<String>,
    reset_available: Option<u64>,
    reset_count_at: Option<u64>,
    reset_count_error: Option<String>,
    quota_at: Option<u64>,
    quota_error: Option<String>,
}
impl From<&Credential> for QuotaData {
    fn from(c: &Credential) -> Self {
        Self {
            five: c.five,
            week: c.week,
            reset_five: c.reset_five.clone(),
            reset_week: c.reset_week.clone(),
            quota_items: c.quota_items.clone(),
            plan: c.plan.clone(),
            reset_available: c.reset_available,
            reset_count_at: c.reset_count_at,
            reset_count_error: c.reset_count_error.clone(),
            quota_at: c.quota_at,
            quota_error: c.quota_error.clone(),
        }
    }
}
impl QuotaData {
    pub(super) fn apply(&self, c: &mut Credential) {
        c.five = self.five;
        c.week = self.week;
        c.reset_five = self.reset_five.clone();
        c.reset_week = self.reset_week.clone();
        c.quota_items = self.quota_items.clone();
        if c.plan.is_none() {
            c.plan = self.plan.clone();
        }
        c.reset_available = self.reset_available;
        c.reset_count_at = self.reset_count_at;
        c.reset_count_error = self.reset_count_error.clone();
        c.quota_at = self.quota_at;
        c.quota_error = self.quota_error.clone();
    }
    fn valid(&self) -> bool {
        [self.five, self.week]
            .into_iter()
            .chain(self.quota_items.iter().map(|q| q.remaining))
            .flatten()
            .all(|n| n.is_finite() && (0.0..=100.0).contains(&n))
            && self.quota_at.is_some_and(|t| t > 0 && t <= super::now())
            && self.reset_count_at.is_none_or(|t| t <= super::now())
    }
}
#[derive(Serialize, Deserialize)]
struct Cache {
    version: u8,
    port: u16,
    entries: HashMap<String, QuotaData>,
}
// Opaque lookup key avoids persisting account names. This is an identity check, not encryption.
pub(super) fn identity(r: &Record) -> Option<String> {
    let identified = !r.account_id.is_empty() || r.row.name != r.name;
    // A filename alone cannot distinguish a replacement account.
    if !identified && r.revision.is_empty() {
        return None;
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (
        &r.name,
        &r.auth_index,
        &r.provider,
        &r.account_id,
        &r.project_id,
        &r.row.name,
    )
        .hash(&mut hash);
    if !identified {
        r.revision.hash(&mut hash);
    }
    Some(format!("{:016x}", hash.finish()))
}
fn path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .map(|p| p.join("quota-cache.json"))
        .map_err(|_| "无法定位额度缓存目录".into())
}
pub(super) fn restore(app: &tauri::AppHandle, session: &mut Session) -> Result<(), String> {
    if !session.snapshot.connected {
        return Ok(());
    }
    load(&path(app)?, session)
}
pub(super) fn persist(app: &tauri::AppHandle, session: &Session) -> Result<(), String> {
    save(&path(app)?, session)
}
fn load(path: &Path, session: &mut Session) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(READ_ERROR.into()),
    };
    if !file.metadata().map_err(|_| READ_ERROR)?.is_file() {
        return Err(READ_ERROR.into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| READ_ERROR)?;
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| READ_ERROR)?;
    if bytes.len() as u64 > LIMIT {
        return Err(READ_ERROR.into());
    }
    let cache: Cache = serde_json::from_slice(&bytes).map_err(|_| READ_ERROR)?;
    if cache.version != 1 || cache.entries.values().any(|q| !q.valid()) {
        return Err(READ_ERROR.into());
    }
    if cache.port == session.snapshot.port {
        for record in session.records.values_mut() {
            if let Some(data) = identity(record).and_then(|key| cache.entries.get(&key)) {
                data.apply(&mut record.row);
            }
        }
    }
    Ok(())
}
fn save(path: &Path, session: &Session) -> Result<(), String> {
    let write = || -> Result<(), Box<dyn std::error::Error>> {
        let cache = Cache {
            version: 1,
            port: session.snapshot.port,
            entries: session
                .records
                .values()
                .filter(|r| r.row.quota_at.is_some())
                .filter_map(|r| identity(r).map(|key| (key, QuotaData::from(&r.row))))
                .collect(),
        };
        let bytes = serde_json::to_vec(&cache)?;
        if bytes.len() as u64 > LIMIT {
            return Err("quota cache too large".into());
        }
        let directory = path.parent().ok_or("invalid quota directory")?;
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(directory)?;
        if !std::fs::symlink_metadata(directory)?.file_type().is_dir() {
            return Err("invalid quota directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let mut temp = tempfile::Builder::new()
            .prefix(".quota-")
            .tempfile_in(directory)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(path)?;
        Ok(())
    };
    write().map_err(|_| "额度已更新，但本地缓存保存失败；重开后可能显示此前数据".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        let mut s = Session::default();
        s.snapshot.port = 8317;
        s.records = super::super::normalize(&serde_json::json!({"files":[{
            "name":"synthetic.json", "auth_index":"idx", "provider":"codex",
            "email":"private@example.invalid", "disabled":false,
            "id_token":{"chatgpt_account_id":"private-id"}
        }]}))
        .unwrap();
        s
    }
    #[test]
    fn restart_retains_zero_unknown_windows_errors_and_times_without_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data/quota-cache.json");
        let mut original = session();
        let c = &mut original.records.values_mut().next().unwrap().row;
        c.five = Some(0.0);
        c.week = None;
        c.quota_at = Some(1234);
        c.reset_available = Some(0);
        c.reset_count_at = Some(1200);
        c.reset_count_error = Some("重置次数更新失败".into());
        c.quota_error = Some("HTTP 403".into());
        c.plan = Some("pro".into());
        c.quota_items.push(QuotaItem {
            remaining: Some(42.0),
            label: "月度".into(),
            ..Default::default()
        });
        save(&path, &original).unwrap();
        let bytes = std::fs::read_to_string(&path).unwrap();
        for private in [
            "private@example.invalid",
            "private-id",
            "synthetic.json",
            "delete_token",
            "auth_index",
        ] {
            assert!(!bytes.contains(private));
        }
        let mut restarted = session();
        load(&path, &mut restarted).unwrap();
        let row = &restarted.records.values().next().unwrap().row;
        assert_eq!(row.five, Some(0.0));
        assert_eq!(row.week, None);
        assert_eq!(row.quota_at, Some(1234));
        assert_eq!(row.reset_count_at, Some(1200));
        assert_eq!(row.reset_available, Some(0));
        assert!(row.quota_error.is_some() && row.reset_count_error.is_some());
        assert_eq!(row.plan.as_deref(), Some("pro"));
        assert_eq!(row.quota_items[0].remaining, Some(42.0));
        assert!(restarted.api.is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }
    #[test]
    fn different_port_or_replaced_identity_does_not_restore_old_quota() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quota.json");
        let mut original = session();
        original.records.values_mut().next().unwrap().row.quota_at = Some(1234);
        save(&path, &original).unwrap();
        let mut port = session();
        port.snapshot.port = 12345;
        load(&path, &mut port).unwrap();
        assert!(port.records.values().all(|r| r.row.quota_at.is_none()));
        let mut account = session();
        account.records.values_mut().next().unwrap().account_id = "replacement".into();
        load(&path, &mut account).unwrap();
        assert!(account.records.values().all(|r| r.row.quota_at.is_none()));
    }
    #[test]
    fn missing_damaged_oversized_and_invalid_cache_do_not_change_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quota.json");
        let mut s = session();
        load(&path, &mut s).unwrap();
        for bytes in [
            b"private-malformed".to_vec(),
            vec![b'x'; (LIMIT + 1) as usize],
        ] {
            std::fs::write(&path, &bytes).unwrap();
            assert_eq!(load(&path, &mut s).unwrap_err(), READ_ERROR);
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        let mut bad = session();
        let row = &mut bad.records.values_mut().next().unwrap().row;
        row.quota_at = Some(1234);
        row.five = Some(110.0);
        save(&path, &bad).unwrap();
        assert!(load(&path, &mut s).is_err());
        assert!(s.records.values().all(|r| r.row.quota_at.is_none()));
    }
    #[test]
    fn anonymous_replacement_requires_file_revision_and_never_reuses_old_quota() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quota.json");
        let mut original = session();
        let r = original.records.values_mut().next().unwrap();
        r.account_id.clear();
        r.row.name = r.name.clone();
        r.row.quota_at = Some(1234);
        assert!(identity(r).is_none());
        save(&path, &original).unwrap();
        assert!(
            serde_json::from_slice::<Cache>(&std::fs::read(&path).unwrap())
                .unwrap()
                .entries
                .is_empty()
        );
        original.records.values_mut().next().unwrap().revision = "v1".into();
        save(&path, &original).unwrap();
        let mut replaced = original;
        let r = replaced.records.values_mut().next().unwrap();
        r.row.quota_at = None;
        r.revision = "v2".into();
        load(&path, &mut replaced).unwrap();
        assert!(replaced.records.values().all(|r| r.row.quota_at.is_none()));
        replaced.records.values_mut().next().unwrap().revision = "v1".into();
        load(&path, &mut replaced).unwrap();
        assert!(replaced
            .records
            .values()
            .all(|r| r.row.quota_at == Some(1234)));
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_and_failed_replacement_preserve_old_file() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().join("data");
        let path = directory.join("quota.json");
        let s = session();
        save(&path, &s).unwrap();
        let previous = std::fs::read(&path).unwrap();
        let moved = dir.path().join("moved");
        std::fs::rename(&directory, &moved).unwrap();
        symlink(&moved, &directory).unwrap();
        assert!(save(&path, &s).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), previous);
        let link = dir.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(load(&link, &mut session()).is_err());
        let blocked = moved.join("blocked");
        std::fs::create_dir(&blocked).unwrap();
        assert!(save(&blocked, &s).is_err());
        assert_eq!(std::fs::read_dir(&moved).unwrap().count(), 2);
    }
}
