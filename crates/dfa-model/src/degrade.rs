//! 模型不可达时的降级编排：退避重试 ≤2 → 纯规则判定，
//! 结果统一带「规则判定 · 待人工判断」标记与降级原因；簇与工单状态绝不自动推进。

use dfa_core::{ErrorCode, DEGRADE_LABEL};
use serde::{Deserialize, Serialize};

use crate::errors::ModelError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradeOutcome<T> {
    pub value: T,
    pub degraded: bool,
    pub label: String,
    pub reason: String,
    pub retries: u32,
    pub error_code: String,
}

pub fn degrade<T>(fallback: T, error: &ModelError) -> DegradeOutcome<T> {
    DegradeOutcome {
        value: fallback,
        degraded: true,
        label: DEGRADE_LABEL.to_string(),
        reason: error.kind.hint().to_string(),
        retries: error.retries,
        error_code: ErrorCode::ModelUnreachable.code().to_string(),
    }
}

pub fn success<T>(value: T) -> DegradeOutcome<T> {
    DegradeOutcome {
        value,
        degraded: false,
        label: "模型草稿 · 待人工确认".to_string(),
        reason: String::new(),
        retries: 0,
        error_code: String::new(),
    }
}
