use dfa_cluster::{ClusterSet, EvolutionAction};
use dfa_core::ClusterMember;

fn member(no: &str) -> ClusterMember {
    ClusterMember {
        workorder_no: no.into(),
        device_id: format!("DEV-{no}"),
        model: "M-EXAMPLE".into(),
        firmware: "FW-EXAMPLE".into(),
        purchase_batch: "LOT-EXAMPLE-01".into(),
        ward: "设备科".into(),
        reported_epoch: 1741000000,
        ..ClusterMember::default()
    }
}

#[test]
fn formed_and_joined_leave_audit_records() {
    let mut set = ClusterSet::new();
    set.ingest("K1", member("WO-1"), "REFUSAL_TO_COOL", &[], 100);
    set.ingest("K1", member("WO-2"), "REFUSAL_TO_COOL", &[], 101);
    assert_eq!(set.total_members(), 2);
    assert_eq!(set.records.len(), 2);
    assert_eq!(set.records[0].action, EvolutionAction::Formed);
    assert_eq!(set.records[1].action, EvolutionAction::Joined);
    assert!(set.verify_audit());
}

#[test]
fn merge_members_equal_sum_of_two_clusters() {
    let mut set = ClusterSet::new();
    set.ingest("K1", member("WO-1"), "REFUSAL_TO_COOL", &[], 100);
    set.ingest("K2", member("WO-2"), "NO_RESPONSE", &[], 101);
    let ids: Vec<String> = set.clusters.iter().map(|c| c.cluster_id.clone()).collect();
    let before = set.total_members();
    let rec = set
        .merge(&ids[0], &ids[1], "新证据：同一批次同一故障", 102)
        .unwrap();
    assert_eq!(rec.action, EvolutionAction::Merged);
    assert_eq!(set.total_members(), before);
    assert_eq!(set.clusters.len(), 1);
    assert_eq!(set.clusters[0].members.len(), 2);
    assert!(set.verify_audit());
}

#[test]
fn split_keeps_original_evidence_snapshot_and_audit() {
    let mut set = ClusterSet::new();
    set.ingest("K1", member("WO-1"), "REFUSAL_TO_COOL", &["model=M".to_string()], 100);
    set.ingest("K1", member("WO-2"), "REFUSAL_TO_COOL", &[], 101);
    let id = set.clusters[0].cluster_id.clone();
    let rec = set
        .split(&id, &["WO-2".to_string()], "固件升级后不再同源", 102)
        .unwrap();
    assert_eq!(rec.action, EvolutionAction::Split);
    let child = set.clusters.iter().find(|c| c.cluster_id != id).unwrap();
    assert!(child.evidence.iter().any(|e| e.contains("原簇证据快照")));
    assert!(set.verify_audit());
    assert_eq!(set.audit.for_cluster(&child.cluster_id).len(), 1);
}

#[test]
fn audit_answers_why_this_order_was_merged() {
    let mut set = ClusterSet::new();
    set.ingest("K1", member("WO-1"), "REFUSAL_TO_COOL", &["同型号同批次".to_string()], 100);
    let entries = set.audit.for_cluster(&set.clusters[0].cluster_id);
    assert_eq!(entries.len(), 1);
    let payload = set.records[0].trigger_evidence.clone();
    assert!(payload.contains("形成新簇"));
}
