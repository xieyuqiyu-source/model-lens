use super::{operate, Session, Snapshot};
pub(super) fn group(provider: &str) -> &str {
    match provider {
        "gemini" | "gemini-cli" => "gemini",
        "xai" | "grok" => "grok",
        "codex" | "claude" | "antigravity" => provider,
        _ => "other",
    }
}
fn targets(s: &Session, selected: &str) -> Vec<String> {
    s.records
        .iter()
        .filter(|(_, r)| r.row.supported && group(&r.provider) == selected)
        .map(|(id, _)| id.clone())
        .collect()
}
#[tauri::command]
pub async fn quota_refresh_group(app: tauri::AppHandle, group: String) -> Result<Snapshot, String> {
    if !["codex", "gemini", "claude", "grok", "antigravity", "other"].contains(&group.as_str()) {
        return Err("凭证分组无效".into());
    }
    operate(app, move |s, app| {
        s.reload()?;
        s.refresh_quota_targets(app, targets(s, &group))
    })
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_group_only_targets_supported_members_without_moving_rows() {
        let mut s = Session {
            records: super::super::normalize(&super::super::tests::files(false)).unwrap(),
            ..Session::default()
        };
        let original = s.records.keys().next().unwrap().clone();
        let mut gemini = s.records[&original].clone();
        gemini.provider = "gemini-cli".into();
        gemini.row.supported = true;
        s.records.insert("gemini".into(), gemini.clone());
        gemini.provider = "antigravity".into();
        s.records.insert("antigravity".into(), gemini);
        assert_eq!(targets(&s, "gemini"), vec!["gemini"]);
        assert_eq!(targets(&s, "antigravity"), vec!["antigravity"]);
        s.records.get_mut("gemini").unwrap().row.supported = false;
        assert!(targets(&s, "gemini").is_empty());
        assert_eq!(group("xai"), "grok");
        assert_eq!(group("new-provider"), "other");
    }
}
