use super::*;
use crate::management::Management;
use tauri::Manager;

#[tauri::command]
pub async fn probe_account_samples(
    app: tauri::AppHandle,
    id: String,
    model: String,
    challenges: Vec<Challenge>,
    previous_run_id: Option<u64>,
) -> Result<Batch, String> {
    if id.len() > 1100
        || model.is_empty()
        || model.len() > 512
        || model.chars().any(char::is_control)
        || challenges.len() != 3
        || challenges.iter().any(|c| {
            c.id.len() > 100
                || c.prompt.trim().is_empty()
                || c.prompt.chars().count() > 4000
                || !(292..=332).contains(&c.expected_count)
        })
        || challenges
            .iter()
            .map(|c| c.expected_count)
            .collect::<std::collections::HashSet<_>>()
            .len()
            != 3
    {
        return Err("账号测试参数无效".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Management>();
        let mut job = {
            let mut s = state
                .0
                .try_lock()
                .map_err(|_| "管理操作正在进行，请稍后重试")?;
            validate_retry(&s, &id, previous_run_id)?;
            reserve(&mut s, &id, &model)?;
            let job = job_session(&s, &id);
            s.publish(&app);
            job
        };
        // Provider requests run outside the shared management lock.
        execute(&mut job, &id, &model, challenges, |job| {
            let mut s = state.0.lock().map_err(|_| "管理状态不可用")?;
            sync_job(&mut s, job, &id)?;
            s.publish(&app);
            Ok(())
        })
    })
    .await
    .map_err(|_| "后台探查任务未完成".to_owned())?
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
        || message.chars().count() > 1000
    {
        return Err("测试结果无效".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Management>();
        let mut job = {
            let s = state.0.lock().map_err(|_| "管理状态不可用")?;
            if !s
                .snapshot
                .account_tests
                .get(&id)
                .is_some_and(|t| t.run_id == run_id && t.status == "analyzing")
            {
                return Err("本轮探查已失效".into());
            }
            job_session(&s, &id)
        };
        let result = finish(&mut job, &id, run_id, status, message);
        let mut s = state.0.lock().map_err(|_| "管理状态不可用")?;
        let result = commit_finished(&mut s, &mut job, &id, result);
        s.publish(&app);
        result
    })
    .await
    .map_err(|_| "测试结果未保存".to_owned())?
}
