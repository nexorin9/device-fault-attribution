//! 幂等键与本地提交账本：sha256(cluster_id|workorder_no) 与时间戳、重试次数无关；
//! 同一对输入只允许一条生效记录，失败项可重放且复用原幂等键。

pub use dfa_core::idempotency_key;

use dfa_core::ReceiptOutcome;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub key: String,
    pub cluster_id: String,
    pub workorder_no: String,
    pub outcome: ReceiptOutcome,
    pub retryable: bool,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct SubmissionLedger {
    entries: HashMap<String, LedgerEntry>,
}

impl SubmissionLedger {
    pub fn new() -> Self {
        SubmissionLedger::default()
    }

    /// 返回 (是否新建生效记录, 该记录)。同键二次提交不产生第二张单。
    pub fn submit(
        &mut self,
        cluster_id: &str,
        workorder_no: &str,
        success: bool,
    ) -> (bool, LedgerEntry) {
        let key = idempotency_key(cluster_id, workorder_no);
        if let Some(existing) = self.entries.get(&key) {
            return (false, existing.clone());
        }
        let entry = LedgerEntry {
            key: key.clone(),
            cluster_id: cluster_id.to_string(),
            workorder_no: workorder_no.to_string(),
            outcome: if success {
                ReceiptOutcome::Success
            } else {
                ReceiptOutcome::Retryable
            },
            retryable: !success,
            retry_count: 0,
        };
        self.entries.insert(key, entry.clone());
        (true, entry)
    }

    pub fn active_count(&self) -> usize {
        self.entries.len()
    }

    pub fn retryable_for_cluster(&self, cluster_id: &str) -> Vec<LedgerEntry> {
        let mut out: Vec<LedgerEntry> = self
            .entries
            .values()
            .filter(|e| e.cluster_id == cluster_id && e.retryable)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.workorder_no.cmp(&b.workorder_no));
        out
    }

    /// 重放：只补齐 retryable 项，复用原幂等键；已成功项不重复提交。
    pub fn replay(&mut self, cluster_id: &str) -> Vec<LedgerEntry> {
        let keys: Vec<String> = self.retryable_for_cluster(cluster_id)
            .iter()
            .map(|e| e.key.clone())
            .collect();
        let mut out = Vec::new();
        for key in keys {
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.outcome = ReceiptOutcome::Success;
                entry.retryable = false;
                entry.retry_count += 1;
                out.push(entry.clone());
            }
        }
        out
    }

    pub fn get_by_key(&self, key: &str) -> Option<&LedgerEntry> {
        self.entries.get(key)
    }
}
