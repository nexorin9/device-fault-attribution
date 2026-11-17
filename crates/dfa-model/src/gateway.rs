//! 单一 ModelGateway：业务层只经此网关，不耦合厂商 SDK。
//! 协议只选 OpenAI-compatible；base_url / api_key / model / timeout 全部配置化。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::errors::{ModelError, ModelErrorKind};
use crate::meta::CallMeta;
use crate::schema::{request_body, validate, OutputKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl ModelConfig {
    /// 凭据全部来自环境变量，代码中无硬编码值。
    pub fn from_env() -> Self {
        ModelConfig {
            base_url: std::env::var("MODEL_BASE_URL").unwrap_or_default(),
            api_key: std::env::var("MODEL_API_KEY").unwrap_or_default(),
            model: std::env::var("MODEL_MODEL").unwrap_or_default(),
            timeout_secs: std::env::var("MODEL_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(30),
        }
    }

    pub fn chat_completions_url(&self) -> String {
        format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/')
        )
    }

    pub fn configured(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportFailure {
    Timeout,
    Connection(String),
    Status(u16),
    Body(String),
}

/// 传输层可替换：生产用 reqwest，协议兼容性测试用本地协议替身。
pub trait Transport {
    fn post_chat(&self, request: &ChatRequest) -> Result<ChatResponse, TransportFailure>;
}

pub struct ModelGateway<T: Transport> {
    pub config: ModelConfig,
    pub transport: T,
    /// 每次调用（含降级）都留一条可审计元数据。
    pub metas: Vec<CallMeta>,
    pub last_error: Option<ModelError>,
    /// 测试可关闭真实退避等待（生产恒为 true）。
    pub real_backoff: bool,
}

pub const MAX_RETRIES: u32 = 2;
pub const BACKOFF_BASE_MS: u64 = 200;

impl<T: Transport> ModelGateway<T> {
    pub fn new(config: ModelConfig, transport: T) -> Self {
        ModelGateway {
            config,
            transport,
            metas: Vec::new(),
            last_error: None,
            real_backoff: true,
        }
    }

    /// 请求体与请求头固定结构，协议层不含任何厂商专有字段。
    pub fn build_request(&self, system: &str, user: &str) -> ChatRequest {
        let mut headers = BTreeMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", self.config.api_key),
        );
        ChatRequest {
            url: self.config.chat_completions_url(),
            headers,
            body: request_body(
                &self.config.model,
                system,
                user,
                self.config.timeout_secs,
            ),
            timeout_secs: self.config.timeout_secs,
        }
    }

    pub fn backoff_ms(attempt: u32) -> u64 {
        BACKOFF_BASE_MS * (1u64 << attempt.min(4))
    }

    /// 结构化输出调用：指数退避重试 ≤2 → schema 校验 → 调用元数据落库。
    pub fn complete(
        &mut self,
        kind: OutputKind,
        system: &str,
        user: &str,
    ) -> Result<Value, ModelError> {
        let request = self.build_request(system, user);
        let request_text = serde_json::to_string(&request.body).unwrap_or_default();
        let started = Instant::now();
        let mut attempts: u32 = 0;
        let mut last_failure = ModelError::new(ModelErrorKind::ConnectionFailed, "未发起调用", 0);

        for attempt in 0..=MAX_RETRIES {
            attempts = attempt;
            match self.transport.post_chat(&request) {
                Ok(response) => {
                    let parsed = parse_content(&response.body);
                    let result = match parsed {
                        Ok(value) => match validate(kind, &value) {
                            Ok(()) => Ok(value),
                            Err(violation) => Err(ModelError::new(
                                ModelErrorKind::SchemaViolation,
                                format!("{}:{}", violation.code(), violation.detail()),
                                attempts,
                            )),
                        },
                        Err(e) => Err(e.with_retries(attempts)),
                    };
                    let mut meta = CallMeta::new(
                        &self.config.model,
                        &request_text,
                        &response.body,
                    );
                    meta.elapsed_ms = started.elapsed().as_millis();
                    meta.retries = attempts;
                    meta.degraded = result.is_err();
                    self.metas.push(meta);
                    match result {
                        Ok(v) => {
                            self.last_error = None;
                            return Ok(v);
                        }
                        Err(e) => {
                            // schema 不符 / 不可解析不做无意义重试，直接降级。
                            self.last_error = Some(e.clone());
                            return Err(e);
                        }
                    }
                }
                Err(failure) => {
                    let retryable = classify_failure(&failure);
                    last_failure = ModelError::new(
                        retryable,
                        format!("{failure:?}"),
                        attempts,
                    );
                    if attempts < MAX_RETRIES {
                        let ms = Self::backoff_ms(attempts);
                        if self.real_backoff {
                            sleep(Duration::from_millis(ms));
                        }
                        continue;
                    }
                }
            }
        }

        let mut meta = CallMeta::new(&self.config.model, &request_text, "DEGRADED");
        meta.elapsed_ms = started.elapsed().as_millis();
        meta.retries = attempts;
        meta.degraded = true;
        self.metas.push(meta);
        self.last_error = Some(last_failure.clone());
        Err(last_failure)
    }
}

impl ModelError {
    fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}

fn classify_failure(failure: &TransportFailure) -> ModelErrorKind {
    match failure {
        TransportFailure::Timeout => ModelErrorKind::Timeout,
        TransportFailure::Connection(_) => ModelErrorKind::ConnectionFailed,
        TransportFailure::Status(code) if *code >= 500 => ModelErrorKind::Server5xx,
        TransportFailure::Status(_) => ModelErrorKind::Unparseable,
        TransportFailure::Body(_) => ModelErrorKind::Unparseable,
    }
}

/// 解析 OpenAI-compatible 响应：choices[0].message.content 为 JSON 字符串；
/// 本地协议替身也允许直接回结构化对象。
pub fn parse_content(body: &str) -> Result<Value, ModelError> {
    let outer: Value = serde_json::from_str(body).map_err(|e| {
        ModelError::new(
            ModelErrorKind::Unparseable,
            format!("响应不是合法 JSON: {e}"),
            0,
        )
    })?;
    let content = outer
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"));
    match content {
        Some(Value::String(s)) => serde_json::from_str::<Value>(s).map_err(|e| {
            ModelError::new(
                ModelErrorKind::Unparseable,
                format!("content 不是合法 JSON: {e}"),
                0,
            )
        }),
        Some(v) => Ok(v.clone()),
        None => Ok(outer.clone()),
    }
}
