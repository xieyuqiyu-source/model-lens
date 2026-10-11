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
pub(super) struct LocalKeychain;
#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use security_framework::passwords::{
        delete_generic_password_options, generic_password, set_generic_password_options,
        PasswordOptions,
    };
    const SERVICE: &str = "io.modellens.desktop.management";
    const ACCOUNT: &str = "local-connection";
    const NOT_FOUND: i32 = -25300;
    fn options(service: &str) -> PasswordOptions {
        let mut options = PasswordOptions::new_generic_password(service, ACCOUNT);
        options.set_access_synchronized(Some(false));
        options
    }
    fn load(service: &str) -> Result<Option<SavedConnection>, String> {
        match generic_password(options(service)) {
            Ok(bytes) => decode(&bytes).map(Some),
            Err(e) if e.code() == NOT_FOUND => Ok(None),
            Err(_) => Err("无法读取本机保存的连接，请解锁钥匙串后重试，或重新输入密钥".into()),
        }
    }
    fn save(service: &str, connection: &SavedConnection) -> Result<(), String> {
        let bytes = serde_json::to_vec(connection).map_err(|_| "无法保存本机连接")?;
        set_generic_password_options(&bytes, options(service))
            .map_err(|_| "连接成功，但本机保存失败；重启后仍使用此前保存的连接（若有）".into())
    }
    fn forget(service: &str) -> Result<(), String> {
        match delete_generic_password_options(options(service)) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == NOT_FOUND => Ok(()),
            Err(_) => Err("无法删除本机密钥，已保留当前连接；请解锁钥匙串后重试".into()),
        }
    }
    impl ConnectionStore for LocalKeychain {
        fn load(&self) -> Result<Option<SavedConnection>, String> {
            load(SERVICE)
        }
        fn save(&self, connection: &SavedConnection) -> Result<(), String> {
            save(SERVICE, connection)
        }
        fn forget(&self) -> Result<(), String> {
            forget(SERVICE)
        }
    }
    #[test]
    #[ignore = "仅显式执行：使用独立测试条目验证本机钥匙串，不访问真实连接"]
    fn keychain_roundtrip() {
        let service = format!(
            "io.modellens.test.{}.{}",
            std::process::id(),
            super::super::now()
        );
        let connection = SavedConnection {
            port: 12345,
            key: "synthetic-test-only".into(),
        };
        assert!(load(&service).unwrap().is_none());
        save(&service, &connection).unwrap();
        let restored = load(&service).unwrap().unwrap();
        assert!(restored.port == connection.port && restored.key == connection.key);
        save(
            &service,
            &SavedConnection {
                port: 12346,
                key: "synthetic-replacement".into(),
            },
        )
        .unwrap();
        assert!(load(&service).unwrap().unwrap().port == 12346);
        forget(&service).unwrap();
        assert!(load(&service).unwrap().is_none());
        forget(&service).unwrap();
    }
}
#[cfg(not(target_os = "macos"))]
impl ConnectionStore for LocalKeychain {
    fn load(&self) -> Result<Option<SavedConnection>, String> {
        Ok(None)
    }
    fn save(&self, _: &SavedConnection) -> Result<(), String> {
        Err("此平台尚未支持本地密钥保存".into())
    }
    fn forget(&self) -> Result<(), String> {
        Ok(())
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
        let store = MemoryStore::default();
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
