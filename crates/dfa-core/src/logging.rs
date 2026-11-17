//! 结构化日志（JSON 行）、统一错误码、运行摘要与敏感信息遮蔽。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub ts: i64,
    pub level: String,
    pub module: String,
    pub event: String,
    pub fields: BTreeMap<String, String>,
}

impl LogLine {
    pub fn new(module: &str, event: &str, ts: i64) -> Self {
        LogLine {
            ts,
            level: "INFO".to_string(),
            module: module.to_string(),
            event: event.to_string(),
            fields: BTreeMap::new(),
        }
    }

    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.fields.insert(key.to_string(), value.into());
        self
    }

    pub fn error(mut self) -> Self {
        self.level = "ERROR".to_string();
        self
    }

    /// 落盘前遮蔽：token / api_key / 患者标识不进日志。
    pub fn to_json_line(&self) -> String {
        let mut safe = self.clone();
        for value in safe.fields.values_mut() {
            *value = redact(value);
        }
        safe.event = redact(&safe.event);
        serde_json::to_string(&safe).unwrap_or_else(|_| "{}".to_string())
    }
}

/// 统一错误码表，贯穿 CLI 与窗口文案。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    SourceUnavailable,
    CredentialMissing,
    FieldMissing,
    VocabularyOutOfRange,
    IdempotencyConflict,
    ModelUnreachable,
    IllegalStateTransition,
    CapabilityMissing,
}

impl ErrorCode {
    pub fn code(&self) -> &'static str {
        match self {
            ErrorCode::SourceUnavailable => "ERR_SOURCE_UNAVAILABLE",
            ErrorCode::CredentialMissing => "ERR_CREDENTIAL_MISSING",
            ErrorCode::FieldMissing => "ERR_FIELD_MISSING",
            ErrorCode::VocabularyOutOfRange => "ERR_VOCABULARY_OUT_OF_RANGE",
            ErrorCode::IdempotencyConflict => "ERR_IDEMPOTENCY_CONFLICT",
            ErrorCode::ModelUnreachable => "ERR_MODEL_UNREACHABLE",
            ErrorCode::IllegalStateTransition => "ERR_ILLEGAL_TRANSITION",
            ErrorCode::CapabilityMissing => "ERR_CAPABILITY_MISSING",
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            ErrorCode::SourceUnavailable => "源不可用，显示「工单源不可用」而非「无故障」",
            ErrorCode::CredentialMissing => "检查 .env 中缺失的环境变量，不打印任何密钥值",
            ErrorCode::FieldMissing => "该条判不可比并列出缺失字段名，去台账补录",
            ErrorCode::VocabularyOutOfRange => "标「待人工归类」，拒绝建单，不新建类别",
            ErrorCode::IdempotencyConflict => "同键重复提交不产生第二张单",
            ErrorCode::ModelUnreachable => "退避重试 ≤2 次后降级纯规则并标注待人工判断",
            ErrorCode::IllegalStateTransition => "状态机拒绝并返回原状态",
            ErrorCode::CapabilityMissing => "能力表取不到即禁止建单，出站请求计数为 0",
        }
    }
}

/// 每轮轮询结束的一行摘要，可直接贴进工单 / 周例会材料。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunSummary {
    pub scanned: usize,
    pub added: usize,
    pub recurring: usize,
    pub clusters: usize,
    pub degradations: usize,
    pub outbound_writes: usize,
    pub failed_rows: usize,
    pub truncations: usize,
}

impl RunSummary {
    pub fn line(&self) -> String {
        format!(
            "扫描={} 新增={} 复现={} 成簇={} 降级={} 出站写={} 脏行={} 截断={}",
            self.scanned,
            self.added,
            self.recurring,
            self.clusters,
            self.degradations,
            self.outbound_writes,
            self.failed_rows,
            self.truncations
        )
    }

    pub fn to_log_line(&self, ts: i64) -> String {
        LogLine::new("orchestrator", "run_summary", ts)
            .with("scanned", self.scanned.to_string())
            .with("added", self.added.to_string())
            .with("recurring", self.recurring.to_string())
            .with("clusters", self.clusters.to_string())
            .with("degradations", self.degradations.to_string())
            .with("outbound_writes", self.outbound_writes.to_string())
            .with("failed_rows", self.failed_rows.to_string())
            .to_json_line()
    }
}

/// 敏感信息遮蔽：Bearer 令牌、key/token 赋值、手机号与身份证样式数字串。
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for marker in ["Bearer ", "bearer "] {
        if let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == ',' || c == '"')
                .map(|i| start + i)
                .unwrap_or(out.len());
            out.replace_range(start..end, "[REDACTED]");
        }
    }
    for key in ["api_key", "API_KEY", "token", "TOKEN", "password"] {
        if let Some(pos) = out.find(key) {
            if let Some(eq) = out[pos..].find('=') {
                let start = pos + eq + 1;
                let end = out[start..]
                    .find(|c: char| c.is_whitespace() || c == ',' || c == '&')
                    .map(|i| start + i)
                    .unwrap_or(out.len());
                if end > start {
                    out.replace_range(start..end, "[REDACTED]");
                }
            }
        }
    }
    mask_long_digits(&out)
}

fn mask_long_digits(s: &str) -> String {
    let mut out = String::new();
    let mut run = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            run.push(ch);
        } else {
            if run.len() >= 11 {
                out.push_str("[REDACTED]");
            } else {
                out.push_str(&run);
            }
            run.clear();
            out.push(ch);
        }
    }
    if run.len() >= 11 {
        out.push_str("[REDACTED]");
    } else {
        out.push_str(&run);
    }
    out
}
