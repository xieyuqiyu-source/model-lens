use reqwest::{blocking::Client, redirect::Policy, Method};
use serde_json::Value;
use std::{io::Read, time::Duration};

pub struct Api {
    client: Client,
    port: u16,
    key: String,
    pub legacy: bool,
}
impl Api {
    pub fn new(port: u16, key: String) -> Result<Self, String> {
        if port == 0
            || key.is_empty()
            || key.len() > 8192
            || key.trim() != key
            || key.chars().any(char::is_control)
        {
            return Err("请输入有效端口与管理密钥，密钥不能含首尾空白或控制字符".into());
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(25))
            .build()
            .map_err(|_| "无法初始化本机连接")?;
        Ok(Self {
            client,
            port,
            key,
            legacy: false,
        })
    }
    pub fn call(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value, String> {
        self.call_with_query(method, path, &[], body)
    }
    pub fn call_with_query(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
    ) -> Result<Value, String> {
        self.call_request(method, path, query, body, Duration::from_secs(25))
    }
    pub fn call_with_timeout(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        timeout: Duration,
    ) -> Result<Value, String> {
        self.call_request(method, path, &[], body, timeout)
    }
    fn call_request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
        timeout: Duration,
    ) -> Result<Value, String> {
        let base = if self.legacy {
            "/v0/management"
        } else {
            "/v8/management"
        };
        let path = if self.legacy {
            match path {
                "/credentials" => "/auth-files",
                "/credentials/status" => "/auth-files/status",
                "/requests/api-call" => "/api-call",
                other => other,
            }
        } else {
            path
        };
        let mut url =
            reqwest::Url::parse(&format!("http://127.0.0.1:{}{}{}", self.port, base, path))
                .map_err(|_| "本机管理请求地址无效")?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query.iter().copied());
        }
        let mut request = self
            .client
            .request(method, url)
            .bearer_auth(&self.key)
            .timeout(timeout);
        if let Some(body) = body {
            request = request.json(body);
        }
        Self::send(request)
    }
    pub fn inference(&self, key: &str, body: Option<&Value>) -> Result<Value, String> {
        let (method, path, timeout) = if body.is_some() {
            (Method::POST, "/v1/chat/completions", 120)
        } else {
            (Method::GET, "/v1/models", 25)
        };
        let mut request = self
            .client
            .request(method, format!("http://127.0.0.1:{}{path}", self.port))
            .timeout(Duration::from_secs(timeout));
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        Self::send(request).map_err(|e| {
            if e == "管理认证失败，请核对管理密钥及本机访问权限" {
                "CLIProxy 客户端认证失败，请检查客户端 API Key 配置".into()
            } else {
                e
            }
        })
    }
    fn send(request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
        let response = request
            .send()
            .map_err(|_| "无法连接本机 CLIProxy，或请求已超时".to_owned())?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(match status {
                401 | 403 => "管理认证失败，请核对管理密钥及本机访问权限".into(),
                404 => "HTTP 404".into(),
                429 => "服务请求过于频繁，请稍后重试".into(),
                _ => format!("CLIProxy 返回 HTTP {status}"),
            });
        }
        // Error bodies and credential payloads must never be logged or forwarded to the UI.
        let mut bytes = Vec::new();
        response
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "读取服务响应失败")?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("响应过大，已停止读取".into());
        }
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes).map_err(|_| "服务未返回有效 JSON".into())
    }
    pub fn list(&mut self) -> Result<Value, String> {
        match self.call(Method::GET, "/credentials", None) {
            Err(e) if e == "HTTP 404" && !self.legacy => {
                self.legacy = true;
                self.call(Method::GET, "/credentials", None)
            }
            result => result,
        }
    }
}
