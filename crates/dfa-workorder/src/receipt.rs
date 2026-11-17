//! 逐条回执、退避重放与失败不中断：回执落库并进只增不改审计链；
//! 重放复用原幂等键，已成功项不重复提交。

use dfa_core::{Receipt, ReceiptOutcome, sha256_hex};
use std::collections::HashMap;

pub const MAX_RETRIES: u32 = 2;
pub const BACKOFF_BASE_MS: u64 = 200;
pub const CONNECT_TIMEOUT_SECS: u64 = 10;

pub fn backoff_ms(attempt: u32) -> u64 {
    BACKOFF_BASE_MS * (1u64 << attempt.min(4))
}

#[derive(Debug, Clone, Default)]
pub struct ReceiptLedger {
    entries: HashMap<String, Receipt>,
}

impl ReceiptLedger {
    pub fn new() -> Self {
        ReceiptLedger::default()
    }

    /// 同一 (cluster_id, workorder_no) 在账本中只允许一条生效记录。
    pub fn record(&mut self, receipt: Receipt) -> bool {
        if self.entries.contains_key(&receipt.idempotency_key) {
            return false;
        }
        self.entries.insert(receipt.idempotency_key.clone(), receipt);
        true
    }

    pub fn active_count(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, key: &str) -> Option<&Receipt> {
        self.entries.get(key)
    }

    /// 重放成功后就地更新回执，幂等键保持不变。
    pub fn mark_success(&mut self, key: &str, ok: bool, http_status: i64) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.retry_count += 1;
            if ok {
                entry.outcome = ReceiptOutcome::Success;
                entry.retryable = false;
                entry.http_status = http_status;
            }
        }
    }

    pub fn retryable_for_cluster(&self, cluster_id: &str) -> Vec<Receipt> {
        let mut out: Vec<Receipt> = self
            .entries
            .values()
            .filter(|r| r.cluster_id == cluster_id && r.retryable)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.workorder_no.cmp(&b.workorder_no));
        out
    }

    /// 重放：按簇拉出 retryable 项，复用原幂等键，已成功项不重复提交。
    pub fn replay<F>(&mut self, cluster_id: &str, mut sender: F) -> Vec<Receipt>
    where
        F: FnMut(&Receipt) -> bool,
    {
        let keys: Vec<String> = self
            .retryable_for_cluster(cluster_id)
            .iter()
            .map(|r| r.idempotency_key.clone())
            .collect();
        let mut out = Vec::new();
        for key in keys {
            let original = self.entries.get(&key).cloned().unwrap();
            let ok = sender(&original);
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.retry_count += 1;
                if ok {
                    entry.outcome = ReceiptOutcome::Success;
                    entry.retryable = false;
                    entry.http_status = 200;
                }
                out.push(entry.clone());
            }
        }
        out
    }

    /// 回执落库 + 进只增不改审计链。
    pub fn persist(
        &self,
        store: &mut dfa_store::Store,
        ts: i64,
    ) -> Result<usize, dfa_store::StoreError> {
        let mut n = 0usize;
        for r in self.entries.values() {
            store.conn.execute(
                "INSERT OR REPLACE INTO receipts(workorder_no, cluster_id, outcome, retryable,
                    http_status, body_digest, retry_count, idempotency_key)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    r.workorder_no,
                    r.cluster_id,
                    format!("{:?}", r.outcome),
                    r.retryable,
                    r.http_status,
                    r.body_digest,
                    r.retry_count,
                    r.idempotency_key,
                ],
            )?;
            store.append_audit(
                "dfa-workorder",
                "RECEIPT",
                &r.workorder_no,
                &format!("{:?}|{}|{}", r.outcome, r.http_status, r.idempotency_key),
                &r.cluster_id,
                ts,
            )?;
            n += 1;
        }
        Ok(n)
    }
}

/// 响应体摘要：只留 sha256，避免把院内响应原文写进日志。
pub fn body_digest(body: &str) -> String {
    sha256_hex(body)
}
