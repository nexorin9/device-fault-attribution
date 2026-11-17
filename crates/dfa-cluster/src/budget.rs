//! 确认预算：新簇 4 次、复现簇 1 次；以 cluster_key 跨 tick 携带，
//! max(新预算, 携带预算−1) 保证只增不减。预算耗尽且无人工确认 → 簇保持观察中，
//! 出站请求计数 = 0，簇与工单状态均不变。

use dfa_core::RunSummary;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerKind {
    NewCluster,
    Recurring,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmBudget {
    pub remaining: u32,
    pub carried_from_key: String,
    pub trigger: TriggerKind,
}

impl ConfirmBudget {
    pub fn initial(trigger: TriggerKind, new_cluster: u32, recurring: u32) -> Self {
        ConfirmBudget {
            remaining: match trigger {
                TriggerKind::NewCluster => new_cluster.max(1),
                TriggerKind::Recurring => recurring.max(1),
            },
            carried_from_key: String::new(),
            trigger,
        }
    }

    pub fn exhausted(&self) -> bool {
        self.remaining == 0
    }
}

/// 跨 tick 携带：取 max(新预算, 携带预算−1)，中途新触发只延长不缩短。
pub fn carry(prev: &ConfirmBudget, key: &str, new_budget: u32) -> ConfirmBudget {
    let carried_decrement = prev.remaining.saturating_sub(1);
    ConfirmBudget {
        remaining: new_budget.max(carried_decrement),
        carried_from_key: key.to_string(),
        trigger: prev.trigger,
    }
}

#[derive(Debug, Clone, Default)]
pub struct BudgetLedger {
    budgets: HashMap<String, ConfirmBudget>,
    pub exhausted_notes: Vec<String>,
    pub outbound_writes: usize,
}

impl BudgetLedger {
    pub fn new() -> Self {
        BudgetLedger::default()
    }

    /// 新触发：已有预算按 max 规则延续；全新键按触发类型起算。
    pub fn on_trigger(
        &mut self,
        key: &str,
        trigger: TriggerKind,
        new_cluster: u32,
        recurring: u32,
    ) -> ConfirmBudget {
        let fresh = ConfirmBudget::initial(trigger, new_cluster, recurring);
        let next = match self.budgets.get(key) {
            Some(prev) => carry(prev, key, fresh.remaining),
            None => fresh,
        };
        self.budgets.insert(key.to_string(), next.clone());
        next
    }

    /// 每 tick 消耗一次确认复检机会；耗尽时留一条记录且不推进状态。
    pub fn tick(&mut self, key: &str) -> ConfirmBudget {
        let current = self
            .budgets
            .entry(key.to_string())
            .or_insert_with(|| ConfirmBudget::initial(TriggerKind::NewCluster, 4, 1));
        current.remaining = current.remaining.saturating_sub(1);
        if current.exhausted() {
            self.exhausted_notes.push(format!(
                "预算耗尽：{key} 保持「观察中」，出站请求计数 = 0"
            ));
        }
        current.clone()
    }

    pub fn get(&self, key: &str) -> Option<&ConfirmBudget> {
        self.budgets.get(key)
    }

    pub fn set(&mut self, key: &str, budget: ConfirmBudget) {
        self.budgets.insert(key.to_string(), budget);
    }

    pub fn exhausted(&self, key: &str) -> bool {
        self.budgets
            .get(key)
            .map(|b| b.exhausted())
            .unwrap_or(false)
    }

    /// 人工确认：只有这个动作才允许把出站计数 +1。
    pub fn confirm(&mut self, key: &str) -> bool {
        if self.exhausted(key) {
            return false;
        }
        self.outbound_writes += 1;
        true
    }

    pub fn summary_contribution(&self, summary: &mut RunSummary) {
        summary.degradations += self.exhausted_notes.len();
        summary.outbound_writes += self.outbound_writes;
    }
}
