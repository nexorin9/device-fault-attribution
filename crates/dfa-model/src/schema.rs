//! 三类结构化输出的 JSON schema 校验（归并结果 / 同源判定 / 根因假设与核查步骤）。
//! 字段缺失、类型不符、枚举越界分别给出可区分错误码。

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputKind {
    Merge,
    SameSource,
    RootCause,
}

impl OutputKind {
    pub fn name(&self) -> &'static str {
        match self {
            OutputKind::Merge => "merge_result",
            OutputKind::SameSource => "same_source_verdict",
            OutputKind::RootCause => "root_cause_hypotheses",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaViolation {
    MissingField(String),
    TypeMismatch(String),
    EnumOutOfRange(String),
}

impl SchemaViolation {
    pub fn code(&self) -> &'static str {
        match self {
            SchemaViolation::MissingField(_) => "ERR_SCHEMA_MISSING_FIELD",
            SchemaViolation::TypeMismatch(_) => "ERR_SCHEMA_TYPE_MISMATCH",
            SchemaViolation::EnumOutOfRange(_) => "ERR_SCHEMA_ENUM_OUT_OF_RANGE",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            SchemaViolation::MissingField(s) => s,
            SchemaViolation::TypeMismatch(s) => s,
            SchemaViolation::EnumOutOfRange(s) => s,
        }
    }
}

fn require_string(value: &Value, field: &str) -> Result<(), SchemaViolation> {
    match value.get(field) {
        None => Err(SchemaViolation::MissingField(field.to_string())),
        Some(v) if v.is_string() => Ok(()),
        Some(_) => Err(SchemaViolation::TypeMismatch(format!(
            "{field} 应为 string"
        ))),
    }
}

/// schema 校验器：三类输出共用同一入口，错误码可区分。
pub fn validate(kind: OutputKind, value: &Value) -> Result<(), SchemaViolation> {
    if !value.is_object() {
        return Err(SchemaViolation::TypeMismatch("根对象应为 object".to_string()));
    }
    match kind {
        OutputKind::Merge => {
            require_string(value, "category")?;
            require_string(value, "evidence")?;
        }
        OutputKind::SameSource => {
            let verdict = value
                .get("verdict")
                .ok_or_else(|| SchemaViolation::MissingField("verdict".to_string()))?;
            let s = verdict
                .as_str()
                .ok_or_else(|| SchemaViolation::TypeMismatch("verdict 应为 string".to_string()))?;
            if !matches!(s, "same" | "different" | "insufficient") {
                return Err(SchemaViolation::EnumOutOfRange(format!(
                    "verdict={s} 不在 same/different/insufficient 内"
                )));
            }
            let evidence = value
                .get("evidence")
                .ok_or_else(|| SchemaViolation::MissingField("evidence".to_string()))?;
            if !evidence.is_object() {
                return Err(SchemaViolation::TypeMismatch(
                    "evidence 应为 object".to_string(),
                ));
            }
            for field in ["model", "firmware", "batch", "time_window"] {
                require_string(evidence, field)?;
            }
        }
        OutputKind::RootCause => {
            let hypotheses = value
                .get("hypotheses")
                .ok_or_else(|| SchemaViolation::MissingField("hypotheses".to_string()))?;
            let arr = hypotheses
                .as_array()
                .ok_or_else(|| SchemaViolation::TypeMismatch("hypotheses 应为 array".to_string()))?;
            for h in arr {
                require_string(h, "text")?;
                require_string(h, "confidence")?;
                let confidence = h["confidence"].as_str().unwrap_or("");
                if !matches!(confidence, "high" | "medium" | "low") {
                    return Err(SchemaViolation::EnumOutOfRange(format!(
                        "confidence={confidence} 越界"
                    )));
                }
                let refs = h
                    .get("evidence_refs")
                    .ok_or_else(|| SchemaViolation::MissingField("evidence_refs".to_string()))?;
                if !refs.is_array() {
                    return Err(SchemaViolation::TypeMismatch(
                        "evidence_refs 应为 array".to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// 请求的固定结构：model / messages / response_format=json_object / temperature=0 / timeout。
pub fn request_body(model: &str, system: &str, user: &str, timeout_secs: u64) -> Value {
    serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "response_format": {"type": "json_object"},
        "temperature": 0,
        "timeout": timeout_secs
    })
}
