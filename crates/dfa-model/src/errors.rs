//! 模型错误语义五分类：超时 / 连接失败 / 5xx / 不可解析 / schema 不符。
//! 每类对应可区分错误码与不同的降级提示文案。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelErrorKind {
    Timeout,
    ConnectionFailed,
    Server5xx,
    Unparseable,
    SchemaViolation,
}

impl ModelErrorKind {
    pub fn code(&self) -> &'static str {
        match self {
            ModelErrorKind::Timeout => "ERR_MODEL_TIMEOUT",
            ModelErrorKind::ConnectionFailed => "ERR_MODEL_CONNECTION",
            ModelErrorKind::Server5xx => "ERR_MODEL_5XX",
            ModelErrorKind::Unparseable => "ERR_MODEL_UNPARSEABLE",
            ModelErrorKind::SchemaViolation => "ERR_MODEL_SCHEMA",
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            ModelErrorKind::Timeout => "模型超时：重试 ≤2 次后降级纯规则判定",
            ModelErrorKind::ConnectionFailed => "模型连接失败：降级纯规则判定",
            ModelErrorKind::Server5xx => "模型服务 5xx：降级纯规则判定",
            ModelErrorKind::Unparseable => "响应不可解析：降级纯规则判定",
            ModelErrorKind::SchemaViolation => "结构化输出不符 schema：降级纯规则判定",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelError {
    pub kind: ModelErrorKind,
    pub code: String,
    pub message: String,
    pub retries: u32,
}

impl ModelError {
    pub fn new(kind: ModelErrorKind, message: impl Into<String>, retries: u32) -> Self {
        ModelError {
            kind,
            code: kind.code().to_string(),
            message: message.into(),
            retries,
        }
    }
}
