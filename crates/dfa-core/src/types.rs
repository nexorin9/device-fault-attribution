//! 领域类型：设备、工单、簇与成员、回执、审计条目、班组能力。
//! 字段与 `spec.md`「最小数据格式与字段要求」逐项对齐。

use serde::{Deserialize, Serialize};

use crate::state::ClusterState;

/// 设备台账记录（设备身份真源）。空字符串表示该字段缺失，不猜、不回填。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Device {
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub firmware: String,
    #[serde(default)]
    pub ward: String,
    #[serde(default)]
    pub purchase_batch: String,
}

impl Device {
    /// 缺失字段清单：注册表与硬约束门共用，绝不以默认值兜底。
    pub fn missing_fields(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.serial.trim().is_empty() {
            out.push("serial");
        }
        if self.model.trim().is_empty() {
            out.push("model");
        }
        if self.firmware.trim().is_empty() {
            out.push("firmware");
        }
        if self.purchase_batch.trim().is_empty() {
            out.push("purchase_batch");
        }
        out
    }

    /// 身份可解析（型号 + 序列号双因子齐备）才纳入注册表。
    pub fn identity_complete(&self) -> bool {
        !self.model.trim().is_empty() && !self.serial.trim().is_empty()
    }

    /// 固件与批次齐备才可比；缺一即「不可比」。
    pub fn comparable(&self) -> bool {
        !self.firmware.trim().is_empty() && !self.purchase_batch.trim().is_empty()
    }
}

/// 故障工单（故障现象真源）。`model_hint` 为本地样例的可选双因子校验字段，
/// 线上接口不提供时保持空字符串，不参与任何猜测。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkOrder {
    #[serde(default)]
    pub workorder_no: String,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub fault_text: String,
    #[serde(default)]
    pub fault_code: String,
    #[serde(default)]
    pub reported_at: String,
    /// 判定用时间戳（Unix 秒）。展示用 `reported_at`，判定只用此字段。
    #[serde(default)]
    pub reported_epoch: i64,
    #[serde(default)]
    pub reporter_dept: String,
    #[serde(default)]
    pub model_hint: String,
}

/// 簇成员：工单与权威注册表设备对齐后的证据快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClusterMember {
    #[serde(default)]
    pub workorder_no: String,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub firmware: String,
    #[serde(default)]
    pub purchase_batch: String,
    #[serde(default)]
    pub ward: String,
    #[serde(default)]
    pub fault_text: String,
    #[serde(default)]
    pub reported_at: String,
    #[serde(default)]
    pub reported_epoch: i64,
    #[serde(default)]
    pub fault_category: String,
}

/// 故障簇。模型产出（根因假设 / 核查步骤）恒为草稿，只有人工确认才推进状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cluster {
    pub cluster_id: String,
    pub cluster_key: String,
    pub members: Vec<ClusterMember>,
    pub evidence: Vec<String>,
    pub hypothesis: Vec<String>,
    pub steps: Vec<String>,
    pub state: ClusterState,
    pub fault_category: String,
    pub draft: bool,
    pub earliest_epoch: i64,
    pub first_seen_at: String,
    pub degrade_label: String,
}

impl Cluster {
    pub fn observing(cluster_id: &str, cluster_key: &str, category: &str) -> Self {
        Cluster {
            cluster_id: cluster_id.to_string(),
            cluster_key: cluster_key.to_string(),
            members: Vec::new(),
            evidence: Vec::new(),
            hypothesis: Vec::new(),
            steps: Vec::new(),
            state: ClusterState::Observing,
            fault_category: category.to_string(),
            draft: true,
            earliest_epoch: 0,
            first_seen_at: String::new(),
            degrade_label: String::new(),
        }
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptOutcome {
    Success,
    Failed,
    Retryable,
}

impl ReceiptOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            ReceiptOutcome::Success => "成功",
            ReceiptOutcome::Failed => "失败",
            ReceiptOutcome::Retryable => "可重放",
        }
    }
}

/// 逐条建单回执：单条失败不中断同批，失败项可重放且复用原幂等键。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub workorder_no: String,
    pub cluster_id: String,
    pub outcome: ReceiptOutcome,
    pub retryable: bool,
    pub http_status: i64,
    pub body_digest: String,
    pub retry_count: u32,
    pub idempotency_key: String,
}

/// 只增不改审计条目。entry_hash = sha256(prev_hash + 规范化条目)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub ts: i64,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub payload_digest: String,
    pub prev_hash: String,
    pub entry_hash: String,
    pub cluster_id: String,
}

/// 班组能力类型：班组 → 可处置项集合。每次建单前重取，绝不缓存过期能力。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TeamCapability {
    #[serde(default)]
    pub team: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl TeamCapability {
    pub fn can(&self, item: &str) -> bool {
        self.capabilities.iter().any(|c| c == item)
    }
}
