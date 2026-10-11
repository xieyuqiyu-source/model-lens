use super::{transport::Api, Session};
use serde::{Deserialize, Serialize};

// Only this app's explicitly entered connection is stored; never search other apps' credentials.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct SavedConnection {
    port: u16,
    key: String,
}
pub(super) trait ConnectionStore {
    fn load(&self) -> Result<Option<SavedConnection>, String>;
    fn save(&self, connection: &SavedConnection) -> Result<(), String>;
    fn forget(&self) -> Result<(), String>;
}
pub(super) struct LocalFile {
    path: std::path::PathBuf,
}
impl LocalFile {
    pub(super) fn for_app(app: &tauri::AppHandle) -> Result<Self, String> {
        use tauri::Manager;
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| "无法定位本机连接目录")?;
        Ok(Self {
            path: directory.join("connection.json"),
        })
    }
}
impl ConnectionStore for LocalFile {
    fn load(&self) -> Result<Option<SavedConnection>, String> {
        use std::io::Read;
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = match options.open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("无法读取本机连接文件，请检查数据目录权限或重新输入密钥".into()),
        };
        if !file
            .metadata()
            .map_err(|_| "无法读取本机连接文件")?
            .is_file()
        {
            return Err("本机连接记录无效，请忘记后重新连接".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|_| "无法保护本机连接文件")?;
        }
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| "无法读取本机连接文件")?;
        decode(&bytes).map(Some)
    }
    fn save(&self, connection: &SavedConnection) -> Result<(), String> {
        use std::{fs, io::Write};
        // Write beside the destination, then atomically replace it. A failed write keeps
        // the previous connection intact; temporary files are removed on normal errors.
        let save = || -> Result<(), Box<dyn std::error::Error>> {
            let directory = self.path.parent().ok_or("invalid connection directory")?;
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(directory)?;
            if !fs::symlink_metadata(directory)?.file_type().is_dir() {
                return Err("invalid connection directory".into());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
            }
            let bytes = serde_json::to_vec(connection)?;
            // NamedTempFile is created exclusively with mode 0600 on Unix.
            let mut file = tempfile::Builder::new()
                .prefix(".connection-")
                .tempfile_in(directory)?;
            file.write_all(&bytes)?;
            file.as_file().sync_all()?;
            file.persist(&self.path)?;
            Ok(())
        };
        save()
            .map_err(|_| "连接成功，但本机文件保存失败；重启后仍使用此前保存的连接（若有）".into())
    }
    fn forget(&self) -> Result<(), String> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("无法删除本机连接文件，已保留当前连接；请检查数据目录权限".into()),
        }
    }
}
fn decode(bytes: &[u8]) -> Result<SavedConnection, String> {
    if bytes.len() > 65536 {
        return Err("本机连接记录无效，请忘记后重新连接".into());
    }
    serde_json::from_slice(bytes).map_err(|_| "本机连接记录无效，请忘记后重新连接".into())
}
impl Session {
    pub(super) fn disconnect(&mut self) {
        self.initialized = true;
        self.api = None;
        self.records.clear();
        self.snapshot.connected = false;
        self.snapshot.stale = false;
    }
    fn connect(&mut self, connection: SavedConnection) -> Result<(), String> {
        let api = Api::new(connection.port, connection.key)?;
        self.disconnect();
        self.snapshot.port = connection.port;
        self.api = Some(api);
        if let Err(e) = self.reload() {
            self.api = None;
            return Err(e);
        }
        self.snapshot.connected = true;
        Ok(())
    }
    pub(super) fn connect_and_save(
        &mut self,
        port: u16,
        key: String,
        store: &impl ConnectionStore,
    ) -> Result<(), String> {
        self.initialized = true;
        let connection = SavedConnection { port, key };
        self.connect(connection.clone())?;
        store.save(&connection)?;
        self.snapshot.saved = true;
        Ok(())
    }
    pub(super) fn restore(&mut self, store: &impl ConnectionStore) -> Result<(), String> {
        self.initialized = true;
        if let Some(connection) = store.load()? {
            self.snapshot.saved = true;
            self.connect(connection)?;
        } else {
            self.snapshot.saved = false;
        }
        Ok(())
    }
    pub(super) fn forget(&mut self, store: &impl ConnectionStore) -> Result<(), String> {
        self.initialized = true;
        // Delete first: failure must not falsely report that the saved key is gone.
        store.forget()?;
        self.disconnect();
        self.snapshot.saved = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[test]
    fn local_file_roundtrip_permissions_and_replacement() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("app-data");
        let store = LocalFile {
            path: directory.join("connection.json"),
        };
        assert!(store.load().unwrap().is_none());
        store
            .save(&SavedConnection {
                port: 12345,
                key: "synthetic-one".into(),
            })
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&store.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        store
            .save(&SavedConnection {
                port: 12346,
                key: "synthetic-two".into(),
            })
            .unwrap();
        let restored = store.load().unwrap().unwrap();
        assert!(restored.port == 12346 && restored.key == "synthetic-two");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        store.forget().unwrap();
        assert!(store.load().unwrap().is_none());
        store.forget().unwrap();
    }
    #[test]
    fn damaged_or_oversized_file_is_preserved_and_redacted() {
        let root = tempfile::tempdir().unwrap();
        let store = LocalFile {
            path: root.path().join("connection.json"),
        };
        for bytes in [
            b"synthetic-private-broken-record".to_vec(),
            vec![b'x'; 65537],
        ] {
            std::fs::write(&store.path, &bytes).unwrap();
            assert!(store.load().err().unwrap() == "本机连接记录无效，请忘记后重新连接");
            assert_eq!(std::fs::read(&store.path).unwrap(), bytes);
        }
        store.forget().unwrap();
    }
    #[test]
    fn file_operation_errors_do_not_claim_success_or_leave_temporary_files() {
        let root = tempfile::tempdir().unwrap();
        let store = LocalFile {
            path: root.path().join("connection.json"),
        };
        std::fs::create_dir(&store.path).unwrap();
        assert!(store.load().is_err());
        assert!(store
            .save(&SavedConnection {
                port: 12345,
                key: "synthetic".into()
            })
            .is_err());
        assert!(store.forget().is_err());
        assert!(store.path.is_dir());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_is_not_read_and_failed_save_preserves_previous_record() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("data");
        let store = LocalFile {
            path: directory.join("connection.json"),
        };
        store
            .save(&SavedConnection {
                port: 12345,
                key: "synthetic-old".into(),
            })
            .unwrap();
        let moved = root.path().join("moved");
        std::fs::rename(&directory, &moved).unwrap();
        symlink(&moved, &directory).unwrap();
        assert!(store
            .save(&SavedConnection {
                port: 12346,
                key: "synthetic-new".into()
            })
            .is_err());
        let previous = LocalFile {
            path: moved.join("connection.json"),
        };
        assert!(previous.load().unwrap().unwrap().key == "synthetic-old");
        let link = LocalFile {
            path: root.path().join("link.json"),
        };
        symlink(&previous.path, &link.path).unwrap();
        assert!(link.load().is_err());
        link.forget().unwrap();
        assert!(previous.load().unwrap().is_some());
    }
    #[derive(Default)]
    struct MemoryStore {
        value: RefCell<Option<SavedConnection>>,
        fail_write: bool,
    }
    impl ConnectionStore for MemoryStore {
        fn load(&self) -> Result<Option<SavedConnection>, String> {
            Ok(self.value.borrow().clone())
        }
        fn save(&self, value: &SavedConnection) -> Result<(), String> {
            if self.fail_write {
                return Err("mock save failure".into());
            }
            *self.value.borrow_mut() = Some(value.clone());
            Ok(())
        }
        fn forget(&self) -> Result<(), String> {
            if self.fail_write {
                return Err("mock delete failure".into());
            }
            *self.value.borrow_mut() = None;
            Ok(())
        }
    }
    #[test]
    fn restart_restore_and_forget() {
        let (port, server) = super::super::tests::server(vec![
            (200, super::super::tests::files(false)),
            (200, super::super::tests::files(false)),
        ]);
        let directory = tempfile::tempdir().unwrap();
        let store = LocalFile {
            path: directory.path().join("connection.json"),
        };
        let mut first = Session::default();
        first
            .connect_and_save(port, "synthetic-key".into(), &store)
            .unwrap();
        assert!(first.snapshot.saved && first.snapshot.connected);
        first.disconnect();
        assert!(first.snapshot.saved && !first.snapshot.connected);
        let mut restarted = Session::default();
        restarted.restore(&store).unwrap();
        assert!(restarted.snapshot.connected && restarted.snapshot.saved);
        let snapshot = serde_json::to_string(&restarted.snapshot).unwrap();
        assert!(!snapshot.contains("synthetic-key"));
        restarted.forget(&store).unwrap();
        assert!(
            !restarted.snapshot.connected && !restarted.snapshot.saved && restarted.api.is_none()
        );
        assert!(store.load().unwrap().is_none());
        assert_eq!(server.join().unwrap().len(), 2);
    }
    #[test]
    fn failed_connection_keeps_previous_saved_key() {
        let (port, server) = super::super::tests::server(vec![(401, serde_json::json!({}))]);
        let store = MemoryStore {
            value: RefCell::new(Some(SavedConnection {
                port: 9999,
                key: "old-synthetic".into(),
            })),
            ..MemoryStore::default()
        };
        let mut session = Session::default();
        assert!(session
            .connect_and_save(port, "new-synthetic".into(), &store)
            .is_err());
        assert!(store.load().unwrap().unwrap().key == "old-synthetic");
        assert!(!session.snapshot.connected);
        server.join().unwrap();
    }
    #[test]
    fn failed_restore_keeps_saved_connection_for_retry() {
        let (port, server) = super::super::tests::server(vec![(401, serde_json::json!({}))]);
        let store = MemoryStore {
            value: RefCell::new(Some(SavedConnection {
                port,
                key: "synthetic".into(),
            })),
            ..MemoryStore::default()
        };
        let mut session = Session::default();
        assert!(session.restore(&store).is_err());
        assert!(session.snapshot.saved && !session.snapshot.connected && session.initialized);
        assert!(store.load().unwrap().is_some());
        server.join().unwrap();
    }
    #[test]
    fn storage_failures_do_not_claim_save_or_delete() {
        let (port, server) =
            super::super::tests::server(vec![(200, super::super::tests::files(false))]);
        let store = MemoryStore {
            fail_write: true,
            ..MemoryStore::default()
        };
        let mut session = Session::default();
        assert!(session
            .connect_and_save(port, "synthetic".into(), &store)
            .is_err());
        assert!(session.snapshot.connected && !session.snapshot.saved);
        session.snapshot.saved = true;
        assert!(session.forget(&store).is_err());
        assert!(session.snapshot.connected && session.snapshot.saved);
        server.join().unwrap();
    }
    #[test]
    fn damaged_storage_is_redacted_and_invalid_key_never_connects() {
        assert!(decode(b"private-broken-record")
            .err()
            .unwrap()
            .contains("记录无效"));
        let store = MemoryStore {
            value: RefCell::new(Some(SavedConnection {
                port: 0,
                key: "synthetic".into(),
            })),
            ..MemoryStore::default()
        };
        let mut session = Session::default();
        assert!(session.restore(&store).is_err());
        assert!(session.api.is_none());
    }
}
