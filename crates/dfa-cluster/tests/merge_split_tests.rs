use dfa_cluster::{ClusterSet, ConfirmBudget, TriggerKind};
use dfa_core::ClusterMember;

fn member(no: &str) -> ClusterMember {
    ClusterMember {
        workorder_no: no.into(),
        device_id: format!("DEV-{no}"),
        reported_epoch: 1741000000,
        ..ClusterMember::default()
    }
}

#[test]
fn merged_budget_takes_the_larger_value() {
    let mut set = ClusterSet::new();
    set.ingest("K1", member("WO-1"), "REFUSAL_TO_COOL", &[], 100);
    set.ingest("K2", member("WO-2"), "NO_RESPONSE", &[], 101);
    set.budgets.set(
        "K2",
        ConfirmBudget {
            remaining: 1,
            carried_from_key: String::new(),
            trigger: TriggerKind::Recurring,
        },
    );
    let ids: Vec<String> = set.clusters.iter().map(|c| c.cluster_id.clone()).collect();
    set.merge(&ids[0], &ids[1], "同一批次同一故障", 102).unwrap();
    assert_eq!(set.budgets.get("K1").unwrap().remaining, 4);
}

#[test]
fn split_restarts_child_budget_and_preserves_audit_chain() {
    let mut set = ClusterSet::new();
    set.ingest("K1", member("WO-1"), "REFUSAL_TO_COOL", &["model=M".to_string()], 100);
    set.ingest("K1", member("WO-2"), "REFUSAL_TO_COOL", &[], 101);
    set.budgets.set(
        "K1",
        ConfirmBudget {
            remaining: 0,
            carried_from_key: String::new(),
            trigger: TriggerKind::NewCluster,
        },
    );
    let id = set.clusters[0].cluster_id.clone();
    set.split(&id, &["WO-2".to_string()], "固件升级", 102).unwrap();
    let child_key = set
        .clusters
        .iter()
        .find(|c| c.cluster_id != id)
        .unwrap()
        .cluster_key
        .clone();
    assert_eq!(set.budgets.get(&child_key).unwrap().remaining, 4);
    let entries = set.audit.entries();
    for (i, e) in entries.iter().enumerate() {
        assert_eq!(e.seq, i as u64 + 1);
        if i > 0 {
            assert_eq!(e.prev_hash, entries[i - 1].entry_hash);
        }
    }
    assert!(set.verify_audit());
}
