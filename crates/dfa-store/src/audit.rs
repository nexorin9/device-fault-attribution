//! 只增不改审计链：entry_hash = sha256(prev_hash + 规范化条目)。
//! 接口只提供 append 与查询，**没有 update / delete**。

use dfa_core::{sha256_hex, AuditEntry};
use rusqlite::params;

use crate::db::{Store, StoreError};

pub const GENESIS_HASH: &str = "0";

#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    pub fn new() -> Self {
        AuditLog::default()
    }

    pub fn append(
        &mut self,
        actor: &str,
        action: &str,
        target: &str,
        payload: &str,
        cluster_id: &str,
        ts: i64,
    ) -> AuditEntry {
        let seq = self.entries.len() as u64 + 1;
        let prev_hash = self
            .entries
            .last()
            .map(|e| e.entry_hash.clone())
            .unwrap_or_else(|| GENESIS_HASH.to_string());
        let payload_digest = sha256_hex(payload);
        let canonical = format!(
            "{seq}|{ts}|{actor}|{action}|{target}|{payload_digest}|{prev_hash}|{cluster_id}"
        );
        let entry_hash = sha256_hex(&format!("{prev_hash}{canonical}"));
        let entry = AuditEntry {
            seq,
            ts,
            actor: actor.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            payload_digest,
            prev_hash,
            entry_hash,
            cluster_id: cluster_id.to_string(),
        };
        self.entries.push(entry.clone());
        entry
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn for_cluster(&self, cluster_id: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.cluster_id == cluster_id)
            .collect()
    }

    pub fn for_target(&self, target: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.target == target)
            .collect()
    }

    /// seq 连续 + prev_hash 衔接 + entry_hash 重算一致；任何篡改都会让校验失败。
    pub fn verify_chain(&self) -> bool {
        let mut prev = GENESIS_HASH.to_string();
        for (idx, e) in self.entries.iter().enumerate() {
            if e.seq != idx as u64 + 1 || e.prev_hash != prev {
                return false;
            }
            let canonical = format!(
                "{}|{}|{}|{}|{}|{}|{}|{}",
                e.seq,
                e.ts,
                e.actor,
                e.action,
                e.target,
                e.payload_digest,
                e.prev_hash,
                e.cluster_id
            );
            if e.entry_hash != sha256_hex(&format!("{prev}{canonical}")) {
                return false;
            }
            prev = e.entry_hash.clone();
        }
        true
    }
}

impl Store {
    /// 追加一条审计（唯一写入口），并把条目落 SQLite。
    pub fn append_audit(
        &mut self,
        actor: &str,
        action: &str,
        target: &str,
        payload: &str,
        cluster_id: &str,
        ts: i64,
    ) -> Result<AuditEntry, StoreError> {
        // 直接用库内最后一条计算链头，避免整链重放。
        let (seq, prev_hash): (u64, String) = self.conn.query_row(
            "SELECT seq, entry_hash FROM audit_chain ORDER BY seq DESC LIMIT 1",
            [],
            |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?)),
        ).unwrap_or((0, GENESIS_HASH.to_string()));
        let seq = seq + 1;
        let payload_digest = sha256_hex(payload);
        let canonical = format!(
            "{seq}|{ts}|{actor}|{action}|{target}|{payload_digest}|{prev_hash}|{cluster_id}"
        );
        let entry_hash = sha256_hex(&format!("{prev_hash}{canonical}"));
        let entry = AuditEntry {
            seq,
            ts,
            actor: actor.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            payload_digest,
            prev_hash,
            entry_hash,
            cluster_id: cluster_id.to_string(),
        };
        self.conn.execute(
            "INSERT INTO audit_chain(seq, ts, actor, action, target, payload_digest, prev_hash, entry_hash, cluster_id)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.seq as i64,
                entry.ts,
                entry.actor,
                entry.action,
                entry.target,
                entry.payload_digest,
                entry.prev_hash,
                entry.entry_hash,
                entry.cluster_id,
            ],
        )?;
        Ok(entry)
    }

    pub fn audit_entries(&self) -> Result<Vec<AuditEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, ts, actor, action, target, payload_digest, prev_hash, entry_hash, cluster_id
             FROM audit_chain ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AuditEntry {
                seq: r.get::<_, i64>(0)? as u64,
                ts: r.get(1)?,
                actor: r.get(2)?,
                action: r.get(3)?,
                target: r.get(4)?,
                payload_digest: r.get(5)?,
                prev_hash: r.get(6)?,
                entry_hash: r.get(7)?,
                cluster_id: r.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn audit_for_cluster(&self, cluster_id: &str) -> Result<Vec<AuditEntry>, StoreError> {
        Ok(self
            .audit_entries()?
            .into_iter()
            .filter(|e| e.cluster_id == cluster_id)
            .collect())
    }

    pub fn audit_for_target(&self, target: &str) -> Result<Vec<AuditEntry>, StoreError> {
        Ok(self
            .audit_entries()?
            .into_iter()
            .filter(|e| e.target == target)
            .collect())
    }

    pub fn verify_audit_chain(&self) -> Result<bool, StoreError> {
        let entries = self.audit_entries()?;
        let mut prev = GENESIS_HASH.to_string();
        for (idx, e) in entries.iter().enumerate() {
            if e.seq != idx as u64 + 1 || e.prev_hash != prev {
                return Ok(false);
            }
            let canonical = format!(
                "{}|{}|{}|{}|{}|{}|{}|{}",
                e.seq,
                e.ts,
                e.actor,
                e.action,
                e.target,
                e.payload_digest,
                e.prev_hash,
                e.cluster_id
            );
            if e.entry_hash != sha256_hex(&format!("{prev}{canonical}")) {
                return Ok(false);
            }
            prev = e.entry_hash.clone();
        }
        Ok(true)
    }

    /// 全表扫描：用于「患者可识别标识 0 命中」的验收。
    pub fn dump_all_text(&self) -> Result<String, StoreError> {
        let tables = self.table_names()?;
        let mut out = String::new();
        for table in tables {
            let mut stmt = self.conn.prepare(&format!("SELECT * FROM {table}"))?;
            let count = stmt.column_count();
            let rows = stmt.query_map([], |r| {
                let mut line = String::new();
                for i in 0..count {
                    let v: rusqlite::types::Value = r.get(i)?;
                    match v {
                        rusqlite::types::Value::Text(s) => line.push_str(&s),
                        rusqlite::types::Value::Integer(n) => line.push_str(&n.to_string()),
                        rusqlite::types::Value::Real(n) => line.push_str(&n.to_string()),
                        _ => {}
                    }
                    line.push(' ');
                }
                Ok(line)
            })?;
            for row in rows {
                out.push_str(&row?);
                out.push('\n');
            }
        }
        Ok(out)
    }
}
