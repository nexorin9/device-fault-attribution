//! 簇演进：形成 / 并入 / 合并 / 拆分。每次演进都写审计条目，
//! 使「这张工单当时为何被并进这个簇」可回答；拆分保留被拆出成员的证据快照。

use dfa_core::{Cluster, ClusterMember, ClusterState};
use dfa_store::AuditLog;
use serde::{Deserialize, Serialize};

use crate::budget::{BudgetLedger, ConfirmBudget, TriggerKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvolutionAction {
    Formed,
    Joined,
    Merged,
    Split,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionRecord {
    pub action: EvolutionAction,
    pub before_key: String,
    pub after_key: String,
    pub members_added: Vec<String>,
    pub members_removed: Vec<String>,
    pub trigger_evidence: String,
    pub actor: String,
    pub ts: i64,
}

#[derive(Debug, Clone)]
pub struct ClusterSet {
    pub clusters: Vec<Cluster>,
    pub records: Vec<EvolutionRecord>,
    pub audit: AuditLog,
    pub budgets: BudgetLedger,
    next_id: usize,
}

impl Default for ClusterSet {
    fn default() -> Self {
        ClusterSet::new()
    }
}

impl ClusterSet {
    pub fn new() -> Self {
        ClusterSet {
            clusters: Vec::new(),
            records: Vec::new(),
            audit: AuditLog::new(),
            budgets: BudgetLedger::new(),
            next_id: 1,
        }
    }

    fn next_cluster_id(&mut self) -> String {
        let id = format!("C-{:03}", self.next_id);
        self.next_id += 1;
        id
    }

    fn log(&mut self, record: &EvolutionRecord, target: &str) {
        let payload = format!(
            "{:?}|{}|{}|added={}|removed={}|{}",
            record.action,
            record.before_key,
            record.after_key,
            record.members_added.join(","),
            record.members_removed.join(","),
            record.trigger_evidence
        );
        self.audit.append(
            &record.actor,
            &format!("{:?}", record.action),
            target,
            &payload,
            target,
            record.ts,
        );
        self.records.push(record.clone());
    }

    pub fn find_index_by_key(&self, key: &str) -> Option<usize> {
        self.clusters.iter().position(|c| c.cluster_key == key)
    }

    pub fn find_index_by_id(&self, id: &str) -> Option<usize> {
        self.clusters.iter().position(|c| c.cluster_id == id)
    }

    /// 新证据满足同一 cluster_key → 并入或新建簇。
    pub fn ingest(
        &mut self,
        key: &str,
        member: ClusterMember,
        category: &str,
        evidence: &[String],
        ts: i64,
    ) -> EvolutionRecord {
        let idx = self.find_index_by_key(key);
        let record = if let Some(i) = idx {
            let target = self.clusters[i].cluster_id.clone();
            if !self.clusters[i]
                .members
                .iter()
                .any(|m| m.workorder_no == member.workorder_no)
            {
                self.clusters[i].members.push(member.clone());
            }
            for e in evidence {
                if !self.clusters[i].evidence.contains(e) {
                    self.clusters[i].evidence.push(e.clone());
                }
            }
            self.budgets.on_trigger(key, TriggerKind::Recurring, 4, 1);
            let rec = EvolutionRecord {
                action: EvolutionAction::Joined,
                before_key: key.to_string(),
                after_key: key.to_string(),
                members_added: vec![member.workorder_no.clone()],
                members_removed: Vec::new(),
                trigger_evidence: format!("同 cluster_key 复现：{key}"),
                actor: "dfa-cluster".to_string(),
                ts,
            };
            self.log(&rec, &target);
            rec
        } else {
            let id = self.next_cluster_id();
            let mut cluster = Cluster::observing(&id, key, category);
            cluster.members.push(member.clone());
            cluster.evidence = evidence.to_vec();
            cluster.earliest_epoch = member.reported_epoch;
            cluster.first_seen_at = member.reported_at.clone();
            self.clusters.push(cluster);
            self.budgets.on_trigger(key, TriggerKind::NewCluster, 4, 1);
            let rec = EvolutionRecord {
                action: EvolutionAction::Formed,
                before_key: "-".to_string(),
                after_key: key.to_string(),
                members_added: vec![member.workorder_no.clone()],
                members_removed: Vec::new(),
                trigger_evidence: format!("形成新簇：{key}"),
                actor: "dfa-cluster".to_string(),
                ts,
            };
            self.log(&rec, &id);
            rec
        };
        record
    }

    /// 两个簇因新证据同源而合并：成员与证据快照合并，预算取较大值延续。
    pub fn merge(
        &mut self,
        a_id: &str,
        b_id: &str,
        trigger_evidence: &str,
        ts: i64,
    ) -> Option<EvolutionRecord> {
        let ia = self.find_index_by_id(a_id)?;
        let ib = self.find_index_by_id(b_id)?;
        if ia == ib {
            return None;
        }
        let b = self.clusters[ib].clone();
        let before_key = b.cluster_key.clone();
        let after_key = self.clusters[ia].cluster_key.clone();
        let mut added = Vec::new();
        for m in &b.members {
            if !self.clusters[ia]
                .members
                .iter()
                .any(|x| x.workorder_no == m.workorder_no)
            {
                self.clusters[ia].members.push(m.clone());
                added.push(m.workorder_no.clone());
            }
        }
        for e in &b.evidence {
            if !self.clusters[ia].evidence.contains(e) {
                self.clusters[ia].evidence.push(e.clone());
            }
        }
        let budget_a = self
            .budgets
            .get(&after_key)
            .cloned()
            .unwrap_or_else(|| ConfirmBudget::initial(TriggerKind::NewCluster, 4, 1));
        let budget_b = self
            .budgets
            .get(&before_key)
            .cloned()
            .unwrap_or_else(|| ConfirmBudget::initial(TriggerKind::NewCluster, 4, 1));
        self.budgets.set(
            &after_key,
            ConfirmBudget {
                remaining: budget_a.remaining.max(budget_b.remaining),
                carried_from_key: before_key.clone(),
                trigger: TriggerKind::NewCluster,
            },
        );
        self.clusters.remove(ib);
        let rec = EvolutionRecord {
            action: EvolutionAction::Merged,
            before_key,
            after_key,
            members_added: added,
            members_removed: Vec::new(),
            trigger_evidence: trigger_evidence.to_string(),
            actor: "dfa-cluster".to_string(),
            ts,
        };
        self.log(&rec, a_id);
        Some(rec)
    }

    /// 簇内成员不再满足同源约束时拆出独立簇，被拆出成员保留原簇证据快照（可回查）。
    pub fn split(
        &mut self,
        cluster_id: &str,
        member_nos: &[String],
        trigger_evidence: &str,
        ts: i64,
    ) -> Option<EvolutionRecord> {
        let idx = self.find_index_by_id(cluster_id)?;
        let original = self.clusters[idx].clone();
        let removed: Vec<ClusterMember> = original
            .members
            .iter()
            .filter(|m| member_nos.contains(&m.workorder_no))
            .cloned()
            .collect();
        if removed.is_empty() {
            return None;
        }
        self.clusters[idx]
            .members
            .retain(|m| !member_nos.contains(&m.workorder_no));
        let new_key = format!("{}|SPLIT:{}", original.cluster_key, member_nos.join(","));
        let new_id = self.next_cluster_id();
        let mut child = Cluster::observing(&new_id, &new_key, &original.fault_category);
        child.members = removed.clone();
        child.evidence = original
            .evidence
            .iter()
            .map(|e| format!("原簇证据快照|{cluster_id}|{e}"))
            .collect();
        child.evidence.push(format!("拆分触发：{trigger_evidence}"));
        child.earliest_epoch = removed.iter().map(|m| m.reported_epoch).min().unwrap_or(0);
        child.first_seen_at = removed
            .first()
            .map(|m| m.reported_at.clone())
            .unwrap_or_default();
        child.state = ClusterState::Observing;
        self.clusters.push(child);
        // 拆分后各子簇按触发类型重新起算，不继承已耗尽的预算。
        self.budgets
            .on_trigger(&new_key, TriggerKind::NewCluster, 4, 1);
        self.budgets
            .on_trigger(&original.cluster_key, TriggerKind::NewCluster, 4, 1);
        let rec = EvolutionRecord {
            action: EvolutionAction::Split,
            before_key: original.cluster_key.clone(),
            after_key: new_key,
            members_added: Vec::new(),
            members_removed: removed.iter().map(|m| m.workorder_no.clone()).collect(),
            trigger_evidence: trigger_evidence.to_string(),
            actor: "dfa-cluster".to_string(),
            ts,
        };
        self.log(&rec, &new_id);
        Some(rec)
    }

    pub fn verify_audit(&self) -> bool {
        self.audit.verify_chain()
    }

    pub fn total_members(&self) -> usize {
        self.clusters.iter().map(|c| c.members.len()).sum()
    }
}
