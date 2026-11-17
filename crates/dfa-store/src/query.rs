//! 按 cluster_id 反查簇、成员、确认记录与开单回执，供「为何并进这个簇」回查。

use serde::{Deserialize, Serialize};
use rusqlite::params;

use crate::db::{Store, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClusterDetail {
    pub cluster_id: String,
    pub cluster_key: String,
    pub state: String,
    pub fault_category: String,
    pub members: Vec<String>,
    pub confirmations: Vec<String>,
    pub receipts: Vec<String>,
    pub audit_entries: Vec<String>,
}

impl Store {
    pub fn upsert_cluster(
        &self,
        cluster_id: &str,
        cluster_key: &str,
        state: &str,
        category: &str,
        first_seen_at: &str,
        earliest_epoch: i64,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO clusters(cluster_id, cluster_key, state, fault_category, draft, first_seen_at, earliest_epoch)
             VALUES(?1, ?2, ?3, ?4, 1, ?5, ?6)
             ON CONFLICT(cluster_id) DO UPDATE SET
               cluster_key = excluded.cluster_key,
               state = excluded.state,
               fault_category = excluded.fault_category,
               first_seen_at = excluded.first_seen_at,
               earliest_epoch = excluded.earliest_epoch",
            params![cluster_id, cluster_key, state, category, first_seen_at, earliest_epoch],
        )?;
        Ok(())
    }

    pub fn upsert_member(
        &self,
        cluster_id: &str,
        workorder_no: &str,
        device_id: &str,
        evidence_snapshot: &str,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO cluster_members(cluster_id, workorder_no, device_id, evidence_snapshot)
             VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(cluster_id, workorder_no) DO UPDATE SET
               evidence_snapshot = excluded.evidence_snapshot",
            params![cluster_id, workorder_no, device_id, evidence_snapshot],
        )?;
        Ok(())
    }

    pub fn insert_confirmation(
        &self,
        cluster_id: &str,
        actor: &str,
        ts: i64,
        member_snapshot: &str,
        before_state: &str,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO confirmations(cluster_id, actor, ts, member_snapshot, before_state)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            params![cluster_id, actor, ts, member_snapshot, before_state],
        )?;
        Ok(())
    }

    pub fn cluster_detail(&self, cluster_id: &str) -> Result<ClusterDetail, StoreError> {
        let mut detail = ClusterDetail {
            cluster_id: cluster_id.to_string(),
            ..ClusterDetail::default()
        };
        let mut stmt = self.conn.prepare(
            "SELECT cluster_key, state, fault_category FROM clusters WHERE cluster_id = ?1",
        )?;
        if let Ok(row) = stmt.query_row(params![cluster_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        }) {
            detail.cluster_key = row.0;
            detail.state = row.1;
            detail.fault_category = row.2;
        }
        let mut stmt = self.conn.prepare(
            "SELECT workorder_no || '|' || device_id || '|' || evidence_snapshot
             FROM cluster_members WHERE cluster_id = ?1 ORDER BY workorder_no",
        )?;
        let rows = stmt.query_map(params![cluster_id], |r| r.get::<_, String>(0))?;
        for row in rows {
            detail.members.push(row?);
        }
        let mut stmt = self.conn.prepare(
            "SELECT actor || '@' || ts || '|' || member_snapshot
             FROM confirmations WHERE cluster_id = ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![cluster_id], |r| r.get::<_, String>(0))?;
        for row in rows {
            detail.confirmations.push(row?);
        }
        let mut stmt = self.conn.prepare(
            "SELECT workorder_no || '|' || outcome || '|' || http_status || '|' || idempotency_key
             FROM receipts WHERE cluster_id = ?1 ORDER BY workorder_no",
        )?;
        let rows = stmt.query_map(params![cluster_id], |r| r.get::<_, String>(0))?;
        for row in rows {
            detail.receipts.push(row?);
        }
        for e in self.audit_for_cluster(cluster_id)? {
            detail
                .audit_entries
                .push(format!("#{} {} {} {}", e.seq, e.actor, e.action, e.target));
        }
        Ok(detail)
    }
}
