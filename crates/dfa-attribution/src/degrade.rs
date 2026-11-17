//! 降级编排与「不误推进状态」断言：模型不可达 → 纯规则判定（标「规则判定 · 待人工判断」）；
//! 簇状态、工单状态、审计链中的已确认 / 已开单条目数与降级前快照 diff = 0，出站请求计数 = 0。
//! 混合场景下逐条独立，个别降级不得把整簇标为已确认。

use dfa_core::{RunSummary, DEGRADE_LABEL};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub cluster_states: BTreeMap<String, String>,
    pub workorder_states: BTreeMap<String, String>,
    pub confirmed_entries: usize,
    pub ordered_entries: usize,
    pub outbound_writes: usize,
}

impl StateSnapshot {
    /// 与另一份快照的字段级差异条数。降级前后必须为 0。
    pub fn diff(&self, other: &StateSnapshot) -> usize {
        let mut n = 0usize;
        if self.cluster_states != other.cluster_states {
            n += 1;
        }
        if self.workorder_states != other.workorder_states {
            n += 1;
        }
        if self.confirmed_entries != other.confirmed_entries {
            n += 1;
        }
        if self.ordered_entries != other.ordered_entries {
            n += 1;
        }
        if self.outbound_writes != other.outbound_writes {
            n += 1;
        }
        n
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradeItem<T> {
    pub value: T,
    pub degraded: bool,
    pub label: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct DegradeOrchestrator {
    pub degrade_count: usize,
    pub reasons: Vec<String>,
    /// 降级后仍然产出的结果条数（逐条独立）。
    pub produced: usize,
}

impl DegradeOrchestrator {
    /// 单条：模型成功用模型草稿，失败用规则结果并显式标注降级。
    pub fn run_item<T: Clone>(
        &mut self,
        model_result: Result<T, String>,
        fallback: T,
    ) -> DegradeItem<T> {
        self.produced += 1;
        match model_result {
            Ok(v) => DegradeItem {
                value: v,
                degraded: false,
                label: "模型草稿 · 待人工确认".to_string(),
                reason: String::new(),
            },
            Err(reason) => {
                self.degrade_count += 1;
                self.reasons.push(reason.clone());
                DegradeItem {
                    value: fallback,
                    degraded: true,
                    label: DEGRADE_LABEL.to_string(),
                    reason,
                }
            }
        }
    }

    /// 混合场景：逐条独立，整簇是否被确认由人决定，不由任何一条降级结果决定。
    pub fn cluster_confirmed_from_items(&self) -> bool {
        false
    }

    pub fn write_summary(&self, summary: &mut RunSummary) {
        summary.degradations += self.degrade_count;
        summary.outbound_writes = 0;
    }

    pub fn assert_no_state_advance(before: &StateSnapshot, after: &StateSnapshot) -> bool {
        before.diff(after) == 0 && after.outbound_writes == 0
    }
}
