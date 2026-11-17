//! T12 / T37 proof：同一工单跨 3 个 tick 只入队 1 次、队列顺序稳定。

use dfa_ingest::{EventQueue, IngestEvent};

fn ev(source: &str, key: &str, wo: &str, ts: i64) -> IngestEvent {
    IngestEvent {
        source: source.into(),
        key: key.into(),
        workorder_no: wo.into(),
        reported_epoch: ts,
    }
}

#[test]
fn one_workorder_across_three_ticks_enqueues_once() {
    let mut q = EventQueue::new();
    for tick in 1..=3 {
        assert_eq!(
            q.push(ev("workorders", "WO-EXAMPLE-01", "WO-EXAMPLE-01", tick)),
            tick == 1,
            "第 {tick} tick 的入队结果"
        );
    }
    assert_eq!(q.len(), 1);
    assert_eq!(q.enqueue_calls, 3);
}

#[test]
fn dedup_key_is_source_plus_event_key() {
    let mut q = EventQueue::new();
    assert!(q.push(ev("workorders", "K-1", "WO-1", 10)));
    assert!(!q.push(ev("workorders", "K-1", "WO-1", 11)));
    assert!(q.push(ev("devices", "K-1", "WO-1", 12)), "不同源的同键事件应分别入队");
    assert!(q.push(ev("workorders", "K-2", "WO-2", 13)));
    assert_eq!(q.len(), 3);
}

#[test]
fn sorted_events_are_stable_under_replay() {
    let mut q = EventQueue::new();
    let batch = vec![
        ev("workorders", "K-3", "WO-3", 30),
        ev("devices", "K-1", "WO-1", 10),
        ev("teams", "K-2", "WO-2", 20),
        ev("workorders", "K-4", "WO-1", 10),
    ];
    for e in batch.iter().cloned() {
        q.push(e);
    }
    let a = q.sorted_events();
    let b = q.sorted_events();
    assert_eq!(a, b, "重放同一批事件得到同一序列");
    assert_eq!(a[0].reported_epoch, 10);
    assert_eq!(a[0].workorder_no, "WO-1");
    assert_eq!(a[a.len() - 1].reported_epoch, 30);
    assert!(q.len() == 4 && !q.is_empty());
}
