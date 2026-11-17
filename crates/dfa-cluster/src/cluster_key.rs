//! 同源成簇判定的确定性内核接口：硬约束门 → 受控词表精确 → 别名回退 →
//! 缺字段即不可比；超同源时间窗降级为「需重新观察」，不出同源结论。

use dfa_core::{
    eval_hard_constraints, within_time_window, DeterministicKernel, Device,
    PENDING_MANUAL,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterKeyDecision {
    pub comparable: bool,
    pub missing: Vec<String>,
    pub cluster_key: Option<String>,
    pub category: String,
    pub evidence: Vec<String>,
    pub out_of_vocabulary: bool,
    pub merge_source: dfa_core::MergeSource,
}

impl ClusterKeyDecision {
    pub fn pending_manual() -> Self {
        ClusterKeyDecision {
            comparable: false,
            missing: vec!["fault_category(待人工归类)".to_string()],
            cluster_key: None,
            category: PENDING_MANUAL.to_string(),
            evidence: vec!["词表外描述：不静默新建类别".to_string()],
            out_of_vocabulary: true,
            merge_source: dfa_core::MergeSource::None,
        }
    }
}

/// 判定入口。category_override 供 T24 模型同一化结果接入（与主路径共用同一接口）。
pub fn decide(
    kernel: &DeterministicKernel,
    device: &Device,
    text: &str,
    category_override: Option<&str>,
) -> ClusterKeyDecision {
    let merge = kernel.vocabulary.merge(text);
    let guarded = if let Some(candidate) = category_override {
        kernel.vocabulary.guard(candidate)
    } else {
        dfa_core::GuardResult {
            accepted: merge.in_vocabulary,
            category: merge.category.clone(),
            reason: merge.evidence.clone(),
        }
    };
    let category = if guarded.accepted {
        guarded.category
    } else {
        PENDING_MANUAL.to_string()
    };
    let constraint = eval_hard_constraints(device, &category, &kernel.constraints);
    if !guarded.accepted {
        let mut d = ClusterKeyDecision::pending_manual();
        d.merge_source = merge.source;
        d.evidence.push(guarded.reason.clone());
        return d;
    }
    if !constraint.comparable {
        return ClusterKeyDecision {
            comparable: false,
            missing: constraint.missing.clone(),
            cluster_key: None,
            category,
            evidence: constraint.evidence.clone(),
            out_of_vocabulary: false,
            merge_source: merge.source,
        };
    }
    let mut evidence = constraint.evidence.clone();
    evidence.push(merge.evidence.clone());
    evidence.push(guarded.reason.clone());
    ClusterKeyDecision {
        comparable: true,
        missing: Vec::new(),
        cluster_key: constraint.cluster_key.clone(),
        category,
        evidence,
        out_of_vocabulary: false,
        merge_source: merge.source,
    }
}

/// 簇内最早故障时间超出时间窗 → 「需重新观察」，不出同源结论。
pub fn time_window_ok(
    earliest_epoch: i64,
    latest_epoch: i64,
    kernel: &DeterministicKernel,
) -> bool {
    within_time_window(earliest_epoch, latest_epoch, &kernel.constraints)
}

/// 不可比工单的补录引导：输出待补录字段清单。
pub fn fields_to_replenish(missing: &[String]) -> Vec<String> {
    missing
        .iter()
        .map(|f| format!("请到设备台账补录字段：{f}"))
        .collect()
}
