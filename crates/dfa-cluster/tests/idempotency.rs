use dfa_cluster::{idempotency_key, SubmissionLedger};
use dfa_core::ReceiptOutcome;

#[test]
fn duplicate_submit_only_one_active_record() {
    let mut l = SubmissionLedger::new();
    let (created, _) = l.submit("C-1", "WO-1", true);
    assert!(created);
    let (created2, _) = l.submit("C-1", "WO-1", true);
    assert!(!created2);
    assert_eq!(l.active_count(), 1);
    assert_eq!(l.retryable_for_cluster("C-1").len(), 0);
}

#[test]
fn replay_only_touches_retryable_and_keeps_key() {
    let mut l = SubmissionLedger::new();
    l.submit("C-1", "WO-1", true);
    l.submit("C-1", "WO-2", false);
    let pending = l.retryable_for_cluster("C-1");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].outcome, ReceiptOutcome::Retryable);
    let replayed = l.replay("C-1");
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].key, idempotency_key("C-1", "WO-2"));
    assert_eq!(replayed[0].outcome, ReceiptOutcome::Success);
    assert_eq!(l.active_count(), 2);
    assert!(l.retryable_for_cluster("C-1").is_empty());
}
