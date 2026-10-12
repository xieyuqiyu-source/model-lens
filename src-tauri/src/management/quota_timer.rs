use super::{Management, Session};
use std::time::Duration;
use tauri::Manager;

const INTERVAL: Duration = Duration::from_secs(300);
fn eligible(s: &Session) -> bool {
    s.snapshot.connected
        && !s.snapshot.busy
        && !s
            .snapshot
            .account_tests
            .values()
            .any(super::probe::account::active)
        && s.records.values().any(|r| r.row.supported)
}
pub fn start(app: tauri::AppHandle) {
    // One process-owned timer continues while either WebView is hidden.
    std::thread::spawn(move || loop {
        std::thread::sleep(INTERVAL);
        let state = app.state::<Management>();
        let Ok(mut session) = state.0.try_lock() else {
            continue;
        };
        if !eligible(&session) {
            continue;
        }
        session.snapshot.busy = true;
        session.snapshot.error = None;
        session.publish(&app);
        session.snapshot.error = session.refresh_quotas(&app, None).err();
        session.snapshot.busy = false;
        session.publish(&app);
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timer_never_refreshes_disconnected_busy_or_unsupported_sessions() {
        assert_eq!(INTERVAL.as_secs(), 300);
        let mut s = Session::default();
        s.records = super::super::normalize(&super::super::tests::files(false)).unwrap();
        assert!(!eligible(&s));
        s.snapshot.connected = true;
        assert!(eligible(&s));
        s.snapshot.busy = true;
        assert!(!eligible(&s));
        s.snapshot.busy = false;
        s.snapshot.account_tests.insert(
            "synthetic".into(),
            super::super::probe::account::AccountTest {
                run_id: 1,
                credential_id: "synthetic".into(),
                credential: "synthetic".into(),
                model: "gpt-5.4".into(),
                status: "running".into(),
                message: String::new(),
                at: super::super::now(),
                identity: String::new(),
            },
        );
        assert!(!eligible(&s));
        s.snapshot
            .account_tests
            .get_mut("synthetic")
            .unwrap()
            .status = "unknown".into();
        assert!(eligible(&s));
        s.records.values_mut().for_each(|r| r.row.supported = false);
        assert!(!eligible(&s));
    }
}
