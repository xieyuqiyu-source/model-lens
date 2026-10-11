use super::Session;
use reqwest::Method;
use serde_json::Value;
use std::hash::{Hash, Hasher};
use unicode_normalization::UnicodeNormalization;

// Conservatively treat Unicode compatibility/case variants as collisions too.
fn name_key(name: &str) -> String {
    name.nfkc()
        .flat_map(char::to_lowercase)
        .flat_map(char::to_uppercase)
        .collect::<String>()
        .nfkc()
        .collect()
}
fn same_name(a: &str, b: &str) -> bool {
    name_key(a) == name_key(b)
}

const MAX_IMPORT_BYTES: usize = 1024 * 1024;

fn valid_name(name: &str) -> bool {
    name.len() > 5
        && name.len() <= 240
        && name.trim() == name
        && !name.starts_with('.')
        && name.to_ascii_lowercase().ends_with(".json")
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
}

pub(super) fn file_backed(item: &Value, name: &str) -> bool {
    valid_name(name)
        && !item["runtime_only"].as_bool().unwrap_or(false)
        && item["source"].as_str().is_none_or(|s| s == "file")
}

// Compare only identity, file metadata and enabled state, never tokens or quota counters.
// This is an opaque change marker, not an authorization token or server-side CAS.
pub(super) fn delete_token(item: &Value) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for field in [
        "id",
        "name",
        "auth_index",
        "provider",
        "type",
        "email",
        "disabled",
        "runtime_only",
        "source",
        "size",
        "modtime",
        "updated_at",
    ] {
        item[field].to_string().hash(&mut hash);
    }
    format!("{:016x}", hash.finish())
}

fn parse_import(name: &str, contents: &str) -> Result<Value, String> {
    if !valid_name(name) {
        return Err("请输入不含目录的 .json 文件名，最多 240 字节".into());
    }
    if contents.is_empty() || contents.len() > MAX_IMPORT_BYTES {
        return Err("JSON 文件不能为空，且不能超过 1 MiB".into());
    }
    let body: Value = serde_json::from_str(contents.trim_start_matches('\u{feff}'))
        .map_err(|_| "JSON 格式无效，请检查原文件；内容未上传".to_owned())?;
    let provider = body.get("type").and_then(Value::as_str).unwrap_or_default();
    if !body.is_object()
        || provider.is_empty()
        || provider.len() > 64
        || !provider
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err("请选择单个 CLIProxy 凭证对象，需包含有效的 type 字段".into());
    }
    Ok(body)
}

impl Session {
    pub(super) fn import_credential(&mut self, name: &str, contents: &str) -> Result<(), String> {
        let body = parse_import(name, contents)?;
        self.reload()?;
        if self.records.values().any(|r| same_name(&r.name, name)) {
            return Err("已存在同名凭证，未覆盖；请更换文件名后重新导入".into());
        }
        let write = self
            .api
            .as_ref()
            .ok_or("请先连接管理接口")?
            .call_with_query(Method::POST, "/credentials", &[("name", name)], Some(&body));
        if self.reload().is_err() {
            return Err("导入结果未确认，请刷新列表核实；不要重复导入".into());
        }
        if write.is_err() {
            return Err("导入请求未获确认，已重新读取列表；没有自动重试，请核实后再操作".into());
        }
        if !self.records.values().any(|r| r.name == name) {
            return Err("服务已响应，但列表中尚未找到导入凭证；请刷新核实，不要重复导入".into());
        }
        Ok(())
    }

    pub(super) fn delete_credential(&mut self, id: &str, expected: &str) -> Result<(), String> {
        self.reload()?;
        let record = self.records.get(id).ok_or("凭证已不存在，列表已更新")?;
        if record.row.delete_token != expected {
            return Err("凭证已被其他界面修改，未删除；请重新确认".into());
        }
        if !record.row.deletable {
            return Err("该凭证没有可删除的独立 JSON 文件，请在原服务中管理".into());
        }
        let name = record.name.clone();
        if self
            .records
            .values()
            .filter(|r| same_name(&r.name, &name))
            .count()
            != 1
        {
            return Err("多个凭证共用此文件，未删除；请在原服务中管理".into());
        }
        let write = self
            .api
            .as_ref()
            .ok_or("请先连接管理接口")?
            .call_with_query(
                Method::DELETE,
                "/credentials",
                &[("name", name.as_str())],
                None,
            );
        if self.reload().is_err() {
            return Err("删除结果未确认，请刷新列表核实；不要重复删除".into());
        }
        if write.is_err() {
            return Err("删除请求未获确认，已重新读取列表；没有自动重试，请核实后再操作".into());
        }
        if self.records.values().any(|r| same_name(&r.name, &name)) {
            return Err("服务已响应，但凭证仍在列表中；未确认删除，请刷新核实".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
