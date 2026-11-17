//! 逐 tick 差分三态（源 `reapply_targets` 语义转义）：
//! None = 新增 / 换槽位重插、Some(p) && !p.online = 离线→在线复现、
//! Some(p) && p.online = 未变。已闭环簇成员不再作为写入目标。

use dfa_core::WorkOrder;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickState {
    pub workorder_no: String,
    /// 上一 tick 是否在线（在场）。false 表示上一轮离线，本轮重连 = 复现。
    pub online: bool,
    pub stable_key: String,
}

pub type PrevSnapshot = HashMap<String, TickState>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeltaKind {
    Added,
    Recurring,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delta {
    pub kind: DeltaKind,
    pub workorder_no: String,
    pub device_id: String,
    pub stable_key: String,
    pub reported_epoch: i64,
}

/// 同 tick 内按 (reported_at, workorder_no) 稳定排序，重放同一快照得到同一序列。
pub fn stable_sort(orders: &mut Vec<WorkOrder>) {
    orders.sort_by(|a, b| {
        a.reported_epoch
            .cmp(&b.reported_epoch)
            .then_with(|| a.reported_at.cmp(&b.reported_at))
            .then_with(|| a.workorder_no.cmp(&b.workorder_no))
    });
}

/// 严格三态差分。差分键 = 工单号 + 设备稳定身份。
pub fn reapply_targets(prev: &PrevSnapshot, next: &[WorkOrder]) -> Vec<Delta> {
    let mut sorted = next.to_vec();
    stable_sort(&mut sorted);
    sorted
        .into_iter()
        .map(|wo| {
            let kind = match prev.get(&wo.workorder_no) {
                None => DeltaKind::Added,
                Some(p) if !p.online => DeltaKind::Recurring,
                Some(_) => DeltaKind::Unchanged,
            };
            let stable_key = prev
                .get(&wo.workorder_no)
                .map(|p| p.stable_key.clone())
                .unwrap_or_else(|| wo.device_id.clone());
            Delta {
                kind,
                workorder_no: wo.workorder_no,
                device_id: wo.device_id,
                stable_key,
                reported_epoch: wo.reported_epoch,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffResult {
    /// 本轮真正的写入目标（不含「未变」与已闭环簇成员）。
    pub targets: Vec<Delta>,
    /// 只计入簇历史的工单号，不新建簇、不重新进入待确认队列。
    pub history: Vec<String>,
    pub counts: DeltaCounts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeltaCounts {
    pub added: usize,
    pub recurring: usize,
    pub unchanged: usize,
    pub historical: usize,
}

/// 写入目标过滤：未变不计费；已闭环簇成员只进历史，不进待确认队列。
pub fn write_targets(deltas: &[Delta], closed_members: &HashSet<String>) -> DiffResult {
    let mut counts = DeltaCounts::default();
    let mut targets = Vec::new();
    let mut history = Vec::new();
    for d in deltas {
        match d.kind {
            DeltaKind::Unchanged => counts.unchanged += 1,
            DeltaKind::Added => {
                if closed_members.contains(&d.workorder_no)
                    || closed_members.contains(&d.device_id)
                {
                    counts.historical += 1;
                    history.push(d.workorder_no.clone());
                } else {
                    counts.added += 1;
                    targets.push(d.clone());
                }
            }
            DeltaKind::Recurring => {
                if closed_members.contains(&d.workorder_no)
                    || closed_members.contains(&d.device_id)
                {
                    counts.historical += 1;
                    history.push(d.workorder_no.clone());
                } else {
                    counts.recurring += 1;
                    targets.push(d.clone());
                }
            }
        }
    }
    DiffResult {
        targets,
        history,
        counts,
    }
}

/// 由本轮工单构造下一 tick 的快照态（全部视为在线）。
pub fn next_snapshot(orders: &[WorkOrder]) -> PrevSnapshot {
    orders
        .iter()
        .map(|wo| {
            (
                wo.workorder_no.clone(),
                TickState {
                    workorder_no: wo.workorder_no.clone(),
                    online: true,
                    stable_key: wo.device_id.clone(),
                },
            )
        })
        .collect()
}
