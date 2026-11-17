//! 硬约束求值器：任一必填字段缺失即「不可比 + 缺失字段名列表」，不以默认值兜底。

use dfa_core::{eval_hard_constraints, ConstraintResult, Device};
use serde::{Deserialize, Serialize};

use crate::loader::Config;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingFieldReport {
    pub comparable: bool,
    pub missing_fields: Vec<String>,
    pub note: String,
}

pub fn evaluate(config: &Config, device: &Device, category: &str) -> ConstraintResult {
    eval_hard_constraints(device, category, &config.hard_constraints())
}

pub fn report(config: &Config, device: &Device, category: &str) -> MissingFieldReport {
    let result = evaluate(config, device, category);
    MissingFieldReport {
        comparable: result.comparable,
        missing_fields: result.missing.clone(),
        note: if result.comparable {
            "硬约束齐备".to_string()
        } else {
            format!("不可比，缺字段：{}", result.missing.join(" / "))
        },
    }
}
