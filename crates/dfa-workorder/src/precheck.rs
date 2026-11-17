//! 写侧第一道闸：任何不满足受控约束的建单必须在出站之前被拦下。
//! 10 个必填字段、受控词表、优先级枚举、关键岗位下限、班组能力表逐项校验。

use dfa_core::{FaultVocabulary, TeamCapability};
use serde::{Deserialize, Serialize};

/// 出站写 body 的 10 个字段，与 spec.md 字段规范逐项一致。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkOrderDraft {
    #[serde(default)]
    pub workorder_no: String,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub device_model: String,
    #[serde(default)]
    pub firmware_version: String,
    #[serde(default)]
    pub fault_code: String,
    #[serde(default)]
    pub fault_text: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub assignee_team: String,
    #[serde(default)]
    pub source_ref: String,
    #[serde(default)]
    pub cluster_id: String,
}

impl WorkOrderDraft {
    pub fn missing_fields(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.workorder_no.trim().is_empty() {
            out.push("workorder_no");
        }
        if self.device_id.trim().is_empty() {
            out.push("device_id");
        }
        if self.device_model.trim().is_empty() {
            out.push("device_model");
        }
        if self.firmware_version.trim().is_empty() {
            out.push("firmware_version");
        }
        if self.fault_code.trim().is_empty() {
            out.push("fault_code");
        }
        if self.fault_text.trim().is_empty() {
            out.push("fault_text");
        }
        if self.priority.trim().is_empty() {
            out.push("priority");
        }
        if self.assignee_team.trim().is_empty() {
            out.push("assignee_team");
        }
        if self.source_ref.trim().is_empty() {
            out.push("source_ref");
        }
        if self.cluster_id.trim().is_empty() {
            out.push("cluster_id");
        }
        out
    }

    pub fn to_body(&self) -> serde_json::Value {
        serde_json::json!({
            "workorder_no": self.workorder_no,
            "device_id": self.device_id,
            "device_model": self.device_model,
            "firmware_version": self.firmware_version,
            "fault_code": self.fault_code,
            "fault_text": self.fault_text,
            "priority": self.priority,
            "assignee_team": self.assignee_team,
            "source_ref": self.source_ref,
            "cluster_id": self.cluster_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecheckInput {
    pub vocabulary: FaultVocabulary,
    /// 每次建单前重取的能力表（绝不缓存）。
    pub capabilities: Vec<TeamCapability>,
    pub critical_ward: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecheckResult {
    pub allowed: bool,
    pub missing_fields: Vec<String>,
    pub reasons: Vec<String>,
}

pub fn precheck(draft: &WorkOrderDraft, input: &PrecheckInput) -> PrecheckResult {
    let mut missing: Vec<String> = draft
        .missing_fields()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut reasons = Vec::new();

    if !draft.fault_code.trim().is_empty()
        && !input.vocabulary.contains(&draft.fault_code)
    {
        reasons.push(format!(
            "词表外 fault_code {} → 待人工归类，拒绝建单，不静默新建类别",
            draft.fault_code
        ));
    }
    if !matches!(draft.priority.as_str(), "P1" | "P2" | "P3" | "P4") {
        reasons.push(format!("priority {} 越界，仅允许 P1/P2/P3/P4", draft.priority));
    }
    if input.critical_ward && matches!(draft.priority.as_str(), "P3" | "P4") {
        reasons.push("关键岗位设备优先级不得低于 P2".to_string());
    }
    let capability_ok = input
        .capabilities
        .iter()
        .any(|t| t.team == draft.assignee_team);
    if !capability_ok {
        reasons.push(format!(
            "建议班组 {} 未命中当次重取的能力表（取不到即禁止建单）",
            draft.assignee_team
        ));
    }
    if !missing.is_empty() {
        reasons.push(format!("缺字段：{}", missing.join(" / ")));
    } else {
        missing.clear();
    }
    PrecheckResult {
        allowed: reasons.is_empty(),
        missing_fields: missing,
        reasons,
    }
}
