use super::{pinned, run, Answer};
use crate::management::{now, Management};
use serde::Serialize;
use tauri::Manager;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTest {
    pub run_id: u64,
    pub model: String,
    pub status: String,
    pub message: String,
    pub at: u64,
    #[serde(skip)]
    pub identity: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    run_id: u64,
    answers: Vec<Answer>,
    error: Option<String>,
}
pub(in crate::management) fn identity(record: &crate::management::Record) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    if !record.account_id.is_empty() || record.row.name != record.name {
        (
            &record.name,
            &record.auth_index,
            &record.provider,
            &record.account_id,
            &record.row.name,
        )
            .hash(&mut hash);
    } else {
        record.row.delete_token.hash(&mut hash);
    }
    format!("{:016x}", hash.finish())
}
#[tauri::command]
pub async fn probe_account_samples(
    app: tauri::AppHandle,
    id: String,
    model: String,
    prompts: Vec<String>,
) -> Result<Batch, String> {
    if id.len() > 1100
        || model.is_empty()
        || model.len() > 512
        || model.chars().any(char::is_control)
        || prompts.len() != 3
        || prompts
            .iter()
            .any(|p| p.trim().is_empty() || p.chars().count() > 4000)
    {
        return Err("账号测试参数无效".into());
    }
    let publish = app.clone();
    run(app, false, move |s| {
        execute(s, &id, &model, prompts, |s| s.publish(&publish))
    })
    .await
}
fn execute(
    s: &mut crate::management::Session,
    id: &str,
    model: &str,
    prompts: Vec<String>,
    mut publish: impl FnMut(&mut crate::management::Session),
) -> Result<Batch, String> {
    if s.snapshot.account_tests.values().any(|t| {
        matches!(t.status.as_str(), "running" | "analyzing") && now().saturating_sub(t.at) < 420
    }) {
        return Err("已有账号正在测试，请等待完成".into());
    }
    let record = s.records.get(id).ok_or("凭证已不存在")?;
    let run_id = s.snapshot.revision;
    s.snapshot.account_tests.insert(
        id.to_owned(),
        AccountTest {
            run_id,
            model: model.to_owned(),
            status: "running".into(),
            message: "采样 0 / 3".into(),
            at: now(),
            identity: identity(record),
        },
    );
    publish(s);
    let mut batch = Batch {
        run_id,
        answers: Vec::new(),
        error: None,
    };
    let result = (|| -> Result<(), String> {
        let record = pinned::prepare(s, &id, &model)?;
        if identity(&record) != s.snapshot.account_tests[id].identity {
            return Err("凭证已变化，请重新测试".into());
        }
        for (i, prompt) in prompts.iter().enumerate() {
            let current = s.snapshot.account_tests.get_mut(id).unwrap();
            current.message = format!("采样 {} / 3", i + 1);
            current.at = now();
            publish(s);
            // Recheck between paid requests; never silently switch to another credential.
            s.reload()?;
            if s.records
                .get(id)
                .is_none_or(|r| identity(r) != identity(&record))
            {
                return Err("凭证已变化，已停止测试".into());
            }
            let answer =
                pinned::sample(s.api.as_ref().ok_or("连接已断开")?, &record, &model, prompt)?;
            let complete = answer.finish == "stop";
            batch.answers.push(answer);
            if !complete {
                return Err("回答未正常结束，已停止后续采样".into());
            }
        }
        Ok(())
    })();
    let test = s.snapshot.account_tests.get_mut(id).unwrap();
    test.at = now();
    match result {
        Ok(()) => {
            test.status = "analyzing".into();
            test.message = "本地分析中…".into();
        }
        Err(e) => {
            test.status = "error".into();
            test.message = e.clone();
            batch.error = Some(e);
        }
    }
    Ok(batch)
}
#[tauri::command]
pub async fn probe_account_finish(
    app: tauri::AppHandle,
    id: String,
    run_id: u64,
    status: String,
    message: String,
) -> Result<(), String> {
    if !["match", "mismatch", "unknown", "error"].contains(&status.as_str())
        || message.chars().count() > 300
    {
        return Err("测试结果无效".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Management>();
        let mut s = state.0.lock().map_err(|_| "管理状态不可用")?;
        let result = finish(&mut s, &id, run_id, status, message);
        s.publish(&app);
        result
    })
    .await
    .map_err(|_| "测试结果未保存".to_owned())?
}

fn finish(
    s: &mut crate::management::Session,
    id: &str,
    run_id: u64,
    status: String,
    message: String,
) -> Result<(), String> {
    if !s
        .snapshot
        .account_tests
        .get(id)
        .is_some_and(|t| t.run_id == run_id && t.status == "analyzing")
    {
        return Err("本轮测试已失效".into());
    }
    if let Err(e) = s.reload() {
        let test = s.snapshot.account_tests.get_mut(id).unwrap();
        test.status = "error".into();
        test.message = "账号状态未能核实，测试结果未应用".into();
        return Err(e);
    }
    let current_identity = s.records.get(id).map(identity);
    let test = s.snapshot.account_tests.get_mut(id).unwrap();
    if current_identity.as_deref() != Some(test.identity.as_str()) {
        test.status = "error".into();
        test.message = "账号已变化，测试结果已丢弃".into();
        return Err(test.message.clone());
    }
    test.status = status;
    test.message = message;
    test.at = now();
    Ok(())
}
#[cfg(test)]
#[path = "account_tests.rs"]
mod tests;
