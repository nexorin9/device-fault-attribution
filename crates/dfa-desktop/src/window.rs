//! 单窗口两栏：左 = 设备注册表（含「待核实」分区），右 = 簇区。
//! 窗口是**纯快照消费者**：只读 dfa-store 快照文件，自身不持有任何 HTTP 客户端，
//! 出站请求计数恒为 0。

use dfa_core::{Cluster, DRAFT_LABEL};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RegistryRow {
    pub device_id: String,
    pub model: String,
    pub firmware: String,
    pub purchase_batch: String,
    pub ward: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PendingRow {
    pub device_id: String,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MemberRow {
    pub device_id: String,
    pub model: String,
    pub firmware: String,
    pub purchase_batch: String,
    pub ward: String,
    pub fault_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClusterCard {
    pub cluster_id: String,
    pub cluster_key: String,
    pub state: String,
    pub member_rows: Vec<MemberRow>,
    pub evidence: Vec<String>,
    pub hypothesis: Vec<String>,
    pub steps: Vec<String>,
    pub draft_label: String,
    pub degrade_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SnapshotView {
    pub health: String,
    pub banner: String,
    pub registry_rows: Vec<RegistryRow>,
    pub pending_rows: Vec<PendingRow>,
    pub cluster_cards: Vec<ClusterCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoColumnLayout {
    pub left_title: String,
    pub left_rows: Vec<String>,
    pub left_pending: Vec<String>,
    pub right_title: String,
    pub right_cards: Vec<String>,
}

impl SnapshotView {
    /// 只读快照文件：窗口从不写库、从不出站。
    pub fn from_snapshot_file(path: &Path) -> Result<SnapshotView, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        SnapshotView::from_snapshot_json(&text)
    }

    pub fn from_snapshot_json(text: &str) -> Result<SnapshotView, String> {
        serde_json::from_str::<SnapshotView>(text).map_err(|e| e.to_string())
    }

    /// 由领域簇与就绪态组装（常驻进程写快照时用）。
    pub fn from_clusters(clusters: &[Cluster], health: &str) -> SnapshotView {
        let banner = if health == "不可用" {
            "工单源不可用".to_string()
        } else {
            format!("工单源{health} · 当前 {} 个待确认簇", clusters.len())
        };
        let mut registry_rows = Vec::new();
        let mut cards = Vec::new();
        for c in clusters {
            for m in &c.members {
                registry_rows.push(RegistryRow {
                    device_id: m.device_id.clone(),
                    model: m.model.clone(),
                    firmware: m.firmware.clone(),
                    purchase_batch: m.purchase_batch.clone(),
                    ward: m.ward.clone(),
                });
            }
            cards.push(ClusterCard {
                cluster_id: c.cluster_id.clone(),
                cluster_key: c.cluster_key.clone(),
                state: c.state.label().to_string(),
                member_rows: c
                    .members
                    .iter()
                    .map(|m| MemberRow {
                        device_id: m.device_id.clone(),
                        model: m.model.clone(),
                        firmware: m.firmware.clone(),
                        purchase_batch: m.purchase_batch.clone(),
                        ward: m.ward.clone(),
                        fault_text: m.fault_text.clone(),
                    })
                    .collect(),
                evidence: c.evidence.clone(),
                hypothesis: c.hypothesis.clone(),
                steps: c.steps.clone(),
                draft_label: DRAFT_LABEL.to_string(),
                degrade_label: c.degrade_label.clone(),
            });
        }
        registry_rows.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        registry_rows.dedup_by(|a, b| a.device_id == b.device_id);
        SnapshotView {
            health: health.to_string(),
            banner,
            registry_rows,
            pending_rows: Vec::new(),
            cluster_cards: cards,
        }
    }

    /// 窗口进程出站请求数：结构上就没有出站通道。
    pub fn outbound_requests(&self) -> usize {
        0
    }

    pub fn pending_count(&self) -> usize {
        self.cluster_cards.len()
    }

    pub fn two_column_layout(&self) -> TwoColumnLayout {
        TwoColumnLayout {
            left_title: "设备注册表".to_string(),
            left_rows: self
                .registry_rows
                .iter()
                .map(|r| format!("{} | {} | {} | {} | {}", r.device_id, r.model, r.firmware, r.purchase_batch, r.ward))
                .collect(),
            left_pending: self
                .pending_rows
                .iter()
                .map(|p| format!("{} | 缺字段 {}", p.device_id, p.missing.join("/")))
                .collect(),
            right_title: "故障簇".to_string(),
            right_cards: self
                .cluster_cards
                .iter()
                .map(|c| format!("{} | {} | {} 台设备", c.cluster_id, c.state, c.member_rows.len()))
                .collect(),
        }
    }

    /// 两栏联动：点设备 → 高亮其所在簇。
    pub fn clusters_for_device(&self, device_id: &str) -> Vec<String> {
        self.cluster_cards
            .iter()
            .filter(|c| c.member_rows.iter().any(|m| m.device_id == device_id))
            .map(|c| c.cluster_id.clone())
            .collect()
    }

    /// 两栏联动：点簇 → 高亮成员设备；待核实分区可查看缺哪个字段。
    pub fn devices_for_cluster(&self, cluster_id: &str) -> Vec<String> {
        self.cluster_cards
            .iter()
            .find(|c| c.cluster_id == cluster_id)
            .map(|c| c.member_rows.iter().map(|m| m.device_id.clone()).collect())
            .unwrap_or_default()
    }

    pub fn missing_for(&self, device_id: &str) -> Vec<String> {
        self.pending_rows
            .iter()
            .find(|p| p.device_id == device_id)
            .map(|p| p.missing.clone())
            .unwrap_or_default()
    }

    /// 终端可打印的两栏文本投影（无显示服务时的真实运行回显）。
    pub fn text_projection(&self) -> String {
        let layout = self.two_column_layout();
        let mut out = String::new();
        out.push_str("┌─ 医疗设备故障归因系统 · 窗口快照投影 ─────────────────────┐\n");
        out.push_str(&format!("│ 状态栏：{}\n", self.banner));
        out.push_str(&format!("│ 左栏 [{}] · {} 台设备 / 待核实 {} 台\n", layout.left_title, layout.left_rows.len(), layout.left_pending.len()));
        for r in layout.left_rows.iter().take(6) {
            out.push_str(&format!("│   {r}\n"));
        }
        for p in &layout.left_pending {
            out.push_str(&format!("│   [待核实] {p}\n"));
        }
        out.push_str(&format!("│ 右栏 [{}] · {} 张簇卡片\n", layout.right_title, layout.right_cards.len()));
        for c in &layout.right_cards {
            out.push_str(&format!("│   {c}\n"));
        }
        out.push_str("└─ 窗口只读快照 · 出站请求 0 ───────────────────────────────┘\n");
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchConfirmInput {
    pub selected_members: Vec<String>,
    pub category: String,
    pub priority: String,
    pub team: String,
    pub second_confirm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmPreview {
    pub order_count: usize,
    pub models: Vec<String>,
    pub cancelled_outbound: usize,
    pub ready: bool,
}

/// 批量确认交互：勾选成员 → 填类别 / 优先级 / 班组 → 二次确认 → 提交逐条回执。
pub fn batch_confirm_preview(input: &BatchConfirmInput) -> ConfirmPreview {
    ConfirmPreview {
        order_count: input.selected_members.len(),
        models: Vec::new(),
        cancelled_outbound: if input.second_confirm { 0 } else { 0 },
        ready: input.second_confirm && !input.selected_members.is_empty(),
    }
}
