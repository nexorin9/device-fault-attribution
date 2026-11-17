//! 确定性判定内核：硬约束门 → 受控词表精确 → 别名回退 → 缺字段即不可比。
//! 这套内核既是产品主路径的兜底，也是模型不可达时的降级路径（不是并列的第二种模式）。

use serde::{Deserialize, Serialize};

use crate::types::Device;
use crate::vocabulary::{FaultVocabulary, MergeOutcome, PENDING_MANUAL};

pub const DEFAULT_TIME_WINDOW_SECS: i64 = 30 * 24 * 3600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardConstraints {
    pub require_model: bool,
    pub require_firmware: bool,
    pub require_batch: bool,
    pub time_window_secs: i64,
}

impl Default for HardConstraints {
    fn default() -> Self {
        HardConstraints {
            require_model: true,
            require_firmware: true,
            require_batch: true,
            time_window_secs: DEFAULT_TIME_WINDOW_SECS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintResult {
    pub comparable: bool,
    pub missing: Vec<String>,
    pub cluster_key: Option<String>,
    pub evidence: Vec<String>,
}

impl ConstraintResult {
    pub fn incomparable(missing: Vec<String>) -> Self {
        ConstraintResult {
            comparable: false,
            missing,
            cluster_key: None,
            evidence: Vec::new(),
        }
    }
}

/// 硬约束求值：任一必填缺失即返回「不可比 + 缺失字段名列表」，不用默认值兜底。
pub fn eval_hard_constraints(
    device: &Device,
    category: &str,
    cfg: &HardConstraints,
) -> ConstraintResult {
    let mut missing = Vec::new();
    if cfg.require_model && device.model.trim().is_empty() {
        missing.push("model".to_string());
    }
    if cfg.require_firmware && device.firmware.trim().is_empty() {
        missing.push("firmware".to_string());
    }
    if cfg.require_batch && device.purchase_batch.trim().is_empty() {
        missing.push("purchase_batch".to_string());
    }
    if category == PENDING_MANUAL {
        missing.push("fault_category(待人工归类)".to_string());
    }
    if !missing.is_empty() {
        return ConstraintResult::incomparable(missing);
    }
    let key = format!(
        "{}|{}|{}|{}",
        device.model.trim(),
        device.firmware.trim(),
        device.purchase_batch.trim(),
        category
    );
    ConstraintResult {
        comparable: true,
        missing: Vec::new(),
        cluster_key: Some(key.clone()),
        evidence: vec![
            format!("model={}", device.model.trim()),
            format!("firmware={}", device.firmware.trim()),
            format!("purchase_batch={}", device.purchase_batch.trim()),
            format!("time_window={}s", cfg.time_window_secs),
            format!("category={category}"),
            format!("cluster_key={key}"),
        ],
    }
}

pub fn within_time_window(earliest_epoch: i64, latest_epoch: i64, cfg: &HardConstraints) -> bool {
    if cfg.time_window_secs <= 0 {
        return true;
    }
    latest_epoch.saturating_sub(earliest_epoch) <= cfg.time_window_secs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SameSourceVerdict {
    Same,
    Different,
    Insufficient,
}

/// 内核一次判定：归并 + 硬约束 + 时间窗，输出必带证据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelDecision {
    pub merge: MergeOutcome,
    pub constraint: ConstraintResult,
    pub category: String,
    pub evidence: Vec<String>,
    /// 降级标记由上层（模型不可达）设置，内核本身不做第二次判定模式。
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicKernel {
    pub vocabulary: FaultVocabulary,
    pub constraints: HardConstraints,
}

impl Default for DeterministicKernel {
    fn default() -> Self {
        DeterministicKernel {
            vocabulary: FaultVocabulary::standard(),
            constraints: HardConstraints::default(),
        }
    }
}

impl DeterministicKernel {
    pub fn evaluate(&self, text: &str, device: &Device, now_epoch: i64) -> KernelDecision {
        let merge = self.vocabulary.merge(text);
        let constraint = eval_hard_constraints(device, &merge.category, &self.constraints);
        let mut evidence = constraint.evidence.clone();
        evidence.push(merge.evidence.clone());
        if constraint.comparable && now_epoch > 0 && device_is_expired(now_epoch, &self.constraints) {
            evidence.push("时间窗：需按簇内最早故障时间复核".to_string());
        }
        KernelDecision {
            category: merge.category.clone(),
            merge,
            constraint,
            evidence,
            degraded: false,
        }
    }
}

fn device_is_expired(_now_epoch: i64, _cfg: &HardConstraints) -> bool {
    // 时间窗以簇内最早故障时间为准，由 dfa-cluster 在成簇时判定；此处只留证据位。
    false
}
