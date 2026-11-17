use dfa_cluster::{
    next_snapshot, reapply_targets, stable_sort, write_targets, DeltaKind, PrevSnapshot,
    TickState,
};
use dfa_core::WorkOrder;
use std::collections::{HashMap, HashSet};

fn wo(no: &str, epoch: i64) -> WorkOrder {
    WorkOrder {
        workorder_no: no.into(),
        device_id: format!("DEV-{no}"),
        reported_epoch: epoch,
        reported_at: format!("t{epoch}"),
        ..WorkOrder::default()
    }
}

#[test]
fn three_states_are_classified() {
    let mut prev: PrevSnapshot = HashMap::new();
    prev.insert(
        "WO-A".into(),
        TickState { workorder_no: "WO-A".into(), online: true, stable_key: "S-A".into() },
    );
    prev.insert(
        "WO-B".into(),
        TickState { workorder_no: "WO-B".into(), online: false, stable_key: "S-B".into() },
    );
    let next = vec![wo("WO-A", 1), wo("WO-B", 2), wo("WO-C", 3)];
    let d = reapply_targets(&prev, &next);
    assert_eq!(d.len(), 3);
    let kinds: Vec<DeltaKind> = d.iter().map(|x| x.kind).collect();
    assert!(kinds.contains(&DeltaKind::Unchanged));
    assert!(kinds.contains(&DeltaKind::Recurring));
    assert!(kinds.contains(&DeltaKind::Added));
}

#[test]
fn stable_sort_is_reproducible() {
    let mut a = vec![wo("WO-C", 3), wo("WO-A", 1), wo("WO-B", 2)];
    stable_sort(&mut a);
    let order: Vec<String> = a.iter().map(|x| x.workorder_no.clone()).collect();
    assert_eq!(order, vec!["WO-A", "WO-B", "WO-C"]);
    let mut b = a.clone();
    stable_sort(&mut b);
    assert_eq!(a, b);
    assert_eq!(reapply_targets(&PrevSnapshot::new(), &a), reapply_targets(&PrevSnapshot::new(), &b));
}

#[test]
fn closed_members_only_go_to_history() {
    let d = reapply_targets(&PrevSnapshot::new(), &[wo("WO-1", 1), wo("WO-2", 2)]);
    let mut closed = HashSet::new();
    closed.insert("WO-1".to_string());
    let r = write_targets(&d, &closed);
    assert_eq!(r.counts.historical, 1);
    assert_eq!(r.targets.len(), 1);
    assert_eq!(r.history, vec!["WO-1".to_string()]);
}

#[test]
fn unchanged_is_never_billed_across_three_ticks() {
    let orders = vec![wo("WO-1", 1)];
    let mut prev = PrevSnapshot::new();
    for _ in 0..3 {
        let d = reapply_targets(&prev, &orders);
        let r = write_targets(&d, &HashSet::new());
        if prev.is_empty() {
            assert_eq!(r.counts.added, 1);
        } else {
            assert_eq!(d[0].kind, DeltaKind::Unchanged);
            assert_eq!(r.targets.len(), 0);
        }
        prev = next_snapshot(&orders);
    }
}
