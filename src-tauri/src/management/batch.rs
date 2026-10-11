use super::{operate, Session, Snapshot};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    id: String,
    delete_token: String,
    enabled: bool,
}
impl Session {
    fn check_target(&self, target: &Target, deleting: bool) -> Result<(), String> {
        let row = &self
            .records
            .get(&target.id)
            .ok_or("凭证已不存在，请刷新列表")?
            .row;
        if row.delete_token != target.delete_token || row.enabled != target.enabled {
            return Err("凭证已被其他界面修改，请重新选择".into());
        }
        if (deleting && !row.deletable) || (!deleting && !row.editable) {
            return Err("选择中包含不可操作的凭证".into());
        }
        Ok(())
    }
    fn batch(
        &mut self,
        targets: &[Target],
        enabled: Option<bool>,
        mut progress: impl FnMut(&mut Self),
    ) -> Result<(), String> {
        let mut ids = HashSet::new();
        if targets.is_empty() || targets.iter().any(|t| !ids.insert(&t.id)) {
            return Err("请选择不重复的凭证".into());
        }
        self.reload()?;
        for target in targets {
            self.check_target(target, enabled.is_none())?;
        }
        for (done, target) in targets.iter().enumerate() {
            let result = (|| {
                self.reload()?;
                self.check_target(target, enabled.is_none())?;
                match enabled {
                    Some(value) if value == target.enabled => Ok(()),
                    Some(value) => self.toggle(
                        &target.id,
                        target.enabled,
                        value,
                        Some(&target.delete_token),
                    ),
                    None => self.delete_credential(&target.id, &target.delete_token),
                }
            })();
            progress(self);
            if let Err(error) = result {
                return Err(format!(
                    "已完成 {done}/{}，本项未确认，后续已停止：{error}",
                    targets.len()
                ));
            }
        }
        Ok(())
    }
}
#[tauri::command]
pub async fn credentials_toggle(
    app: tauri::AppHandle,
    targets: Vec<Target>,
    enabled: bool,
) -> Result<Snapshot, String> {
    operate(app, move |s, app| {
        s.batch(&targets, Some(enabled), |s| s.publish(app))
    })
    .await
}
#[tauri::command]
pub async fn credentials_delete(
    app: tauri::AppHandle,
    targets: Vec<Target>,
) -> Result<Snapshot, String> {
    operate(app, move |s, app| {
        s.batch(&targets, None, |s| s.publish(app))
    })
    .await
}
#[cfg(test)]
mod tests;
