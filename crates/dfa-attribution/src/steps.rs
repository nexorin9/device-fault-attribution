//! 核查步骤排序：回答「先查哪一步最可能证伪」。
//! 排序输入含可证伪优先序、设备停机影响面（关键岗位）与班组当前能力可用性；
//! 不在能力表内的处置项被剔除并记录；同一输入两次排序顺序完全一致。

use dfa_core::TeamCapability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub action: String,
    pub expected: String,
    pub criterion: String,
    pub reason: String,
    /// 数值越小越优先（最可能证伪的排前面）。
    pub falsifiable_rank: u32,
    pub critical_ward: bool,
    pub capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedSteps {
    pub steps: Vec<Step>,
    pub removed: Vec<(String, String)>,
}

impl OrderedSteps {
    pub fn actions(&self) -> Vec<String> {
        self.steps.iter().map(|s| s.action.clone()).collect()
    }
}

/// 能力可用性：处置项必须命中当次重取的班组能力表。
pub fn capability_available(capabilities: &[TeamCapability], item: &str) -> bool {
    capabilities.iter().any(|t| t.can(item))
}

pub fn order_steps(steps: Vec<Step>, capabilities: &[TeamCapability]) -> OrderedSteps {
    let mut kept = Vec::new();
    let mut removed = Vec::new();
    for s in steps {
        if !capability_available(capabilities, &s.capability) {
            removed.push((
                s.action.clone(),
                format!("处置项 {} 不在当次班组能力表内，已剔除", s.capability),
            ));
            continue;
        }
        kept.push(s);
    }
    // 关键岗位 → 可证伪优先序 → 动作名（保证稳定可复现）。
    kept.sort_by(|a, b| {
        let ca = if a.critical_ward { 0 } else { 1 };
        let cb = if b.critical_ward { 0 } else { 1 };
        ca.cmp(&cb)
            .then_with(|| a.falsifiable_rank.cmp(&b.falsifiable_rank))
            .then_with(|| a.action.cmp(&b.action))
    });
    OrderedSteps {
        steps: kept,
        removed,
    }
}
