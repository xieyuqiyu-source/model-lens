pub mod commands;
use super::{pinned, Answer};
use crate::management::{now, Session};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTest {
    pub run_id: u64,
    pub credential_id: String,
    pub credential: String,
    pub model: String,
    pub status: String,
    pub message: String,
    pub at: u64,
    #[serde(skip)]
    pub identity: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Challenge {
    id: String,
    prompt: String,
    expected_count: usize,
}
#[derive(Clone, Serialize)]
pub struct Sample {
    #[serde(flatten)]
    challenge: Challenge,
    #[serde(flatten)]
    answer: Answer,
    slot: usize,
}
#[derive(Clone, Serialize)]
pub struct History {
    #[serde(flatten)]
    pub test: AccountTest,
    pub samples: Vec<Sample>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    run_id: u64,
    samples: Vec<Sample>,
    error: Option<String>,
}
static NEXT_RUN: AtomicU64 = AtomicU64::new(1);
pub(in crate::management) fn active(test: &AccountTest) -> bool {
    test.status == "running" || (test.status == "analyzing" && now().saturating_sub(test.at) < 420)
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

fn remember(s: &mut Session, id: &str) {
    let test = s.snapshot.account_tests[id].clone();
    if let Some(entry) = s
        .snapshot
        .account_history
        .iter_mut()
        .find(|h| h.test.run_id == test.run_id)
    {
        entry.test = test;
    } else {
        s.snapshot.account_history.insert(
            0,
            History {
                test,
                samples: Vec::new(),
            },
        );
        while s.snapshot.account_history.len() > 10 {
            if let Some(i) = s
                .snapshot
                .account_history
                .iter()
                .rposition(|h| !active(&h.test))
            {
                let removed = s.snapshot.account_history.remove(i);
                s.snapshot
                    .account_tests
                    .retain(|_, t| t.run_id != removed.test.run_id || active(t));
            } else {
                break;
            }
        }
    }
}
pub(in crate::management) fn cancel_all(s: &mut Session) {
    for entry in &mut s.snapshot.account_history {
        if active(&entry.test) {
            entry.test.status = "error".into();
            entry.test.message = "连接已断开，已停止后续采样".into();
        }
    }
}
fn validate_retry(s: &Session, id: &str, previous: Option<u64>) -> Result<(), String> {
    if let Some(run_id) = previous {
        if !s.snapshot.account_tests.get(id).is_some_and(|t| {
            t.run_id == run_id && s.records.get(id).is_some_and(|r| identity(r) == t.identity)
        }) {
            return Err("账号或连接已变化，请从当前账号重新探查".into());
        }
    }
    Ok(())
}
fn reserve(s: &mut Session, id: &str, model: &str) -> Result<u64, String> {
    if !s.snapshot.connected || s.snapshot.stale || s.snapshot.busy {
        return Err("管理连接尚未就绪".into());
    }
    if s.snapshot.account_tests.get(id).is_some_and(active) {
        return Err("该账号正在探查".into());
    }
    if s.snapshot
        .account_tests
        .values()
        .filter(|t| active(t))
        .count()
        >= 3
    {
        return Err("最多同时探查 3 个账号".into());
    }
    let r = s.records.get(id).ok_or("凭证已不存在")?;
    let run_id = NEXT_RUN.fetch_add(1, Ordering::Relaxed);
    s.snapshot.account_tests.insert(
        id.into(),
        AccountTest {
            run_id,
            credential_id: id.into(),
            credential: r.row.name.clone(),
            model: model.into(),
            status: "running".into(),
            message: "准备采样".into(),
            at: now(),
            identity: identity(r),
        },
    );
    remember(s, id);
    Ok(run_id)
}
fn job_session(s: &Session, id: &str) -> Session {
    let mut job = Session {
        api: s.api.clone(),
        ..Session::default()
    };
    if let Some(record) = s.records.get(id) {
        job.records.insert(id.into(), record.clone());
    }
    job.snapshot
        .account_tests
        .insert(id.into(), s.snapshot.account_tests[id].clone());
    job.snapshot.account_history = s
        .snapshot
        .account_history
        .iter()
        .filter(|h| h.test.run_id == s.snapshot.account_tests[id].run_id)
        .cloned()
        .collect();
    job
}
fn sync_job(s: &mut Session, job: &Session, id: &str) -> Result<(), String> {
    let test = &job.snapshot.account_tests[id];
    if !s.snapshot.connected
        || !s
            .snapshot
            .account_tests
            .get(id)
            .is_some_and(|t| t.run_id == test.run_id && active(t))
    {
        return Err("本轮探查已失效".into());
    }
    s.snapshot.account_tests.insert(id.into(), test.clone());
    if let Some(entry) = s
        .snapshot
        .account_history
        .iter_mut()
        .find(|h| h.test.run_id == test.run_id)
    {
        *entry = job.snapshot.account_history[0].clone();
    }
    Ok(())
}

fn execute(
    s: &mut Session,
    id: &str,
    model: &str,
    challenges: Vec<Challenge>,
    mut publish: impl FnMut(&Session) -> Result<(), String>,
) -> Result<Batch, String> {
    let run_id = s.snapshot.account_tests[id].run_id;
    let result = (|| -> Result<(), String> {
        let record = pinned::prepare(s, id, model)?;
        if identity(&record) != s.snapshot.account_tests[id].identity {
            return Err("凭证已变化，请重新探查".into());
        }
        for (i, challenge) in challenges.into_iter().enumerate() {
            s.reload()?;
            if s.records
                .get(id)
                .is_none_or(|r| identity(r) != identity(&record))
            {
                return Err("凭证已变化，已停止探查".into());
            }
            let current = s.snapshot.account_tests.get_mut(id).unwrap();
            current.message = format!("采样 {} / 3", i + 1);
            current.at = now();
            remember(s, id);
            publish(s)?;
            let answer = pinned::sample(
                s.api.as_ref().ok_or("连接已断开")?,
                &record,
                model,
                &challenge.prompt,
            )?;
            let complete = answer.finish == "stop";
            s.snapshot.account_history[0].samples.push(Sample {
                challenge,
                answer,
                slot: i + 1,
            });
            if !complete {
                return Err("回答未正常结束，已停止后续采样".into());
            }
        }
        Ok(())
    })();
    let test = s.snapshot.account_tests.get_mut(id).unwrap();
    test.at = now();
    let error = result.err();
    test.status = if error.is_some() {
        "error"
    } else {
        "analyzing"
    }
    .into();
    test.message = error.clone().unwrap_or_else(|| "本地分析中…".into());
    remember(s, id);
    publish(s)?;
    Ok(Batch {
        run_id,
        samples: s.snapshot.account_history[0].samples.clone(),
        error,
    })
}

fn commit_finished(
    s: &mut Session,
    job: &mut Session,
    id: &str,
    mut result: Result<(), String>,
) -> Result<(), String> {
    if s.records
        .get(id)
        .is_none_or(|r| identity(r) != job.snapshot.account_tests[id].identity)
    {
        let test = job.snapshot.account_tests.get_mut(id).unwrap();
        test.status = "error".into();
        test.message = "账号已变化，测试结果已丢弃".into();
        result = Err(test.message.clone());
        remember(job, id);
    }
    sync_job(s, job, id)?;
    result
}

fn finish(
    s: &mut Session,
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
        return Err("本轮探查已失效".into());
    }
    let result = s.reload().and_then(|_| {
        if s.records
            .get(id)
            .is_none_or(|r| identity(r) != s.snapshot.account_tests[id].identity)
        {
            Err("账号已变化，测试结果已丢弃".into())
        } else {
            Ok(())
        }
    });
    let test = s.snapshot.account_tests.get_mut(id).unwrap();
    test.status = if result.is_ok() {
        status
    } else {
        "error".into()
    };
    test.message = if result.is_ok() {
        message
    } else {
        "账号状态未能核实，测试结果未应用".into()
    };
    test.at = now();
    remember(s, id);
    result
}
#[cfg(test)]
#[path = "account_tests.rs"]
mod tests;
