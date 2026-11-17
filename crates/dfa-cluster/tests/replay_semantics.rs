use dfa_cluster::{idempotency_key, SubmissionLedger};
use dfa_core::ReceiptOutcome;

#[test]
fn one_failure_does_not_stop_the_batch() {
    let mut l = SubmissionLedger::new();
    l.submit("C-1", "WO-1", true);
    l.submit("C-1", "WO-2", false);
    l.submit("C-1", "WO-3", true);
    assert_eq!(l.active_count(), 3);
    assert_eq!(l.retryable_for_cluster("C-1").len(), 1);
}

#[test]
fn replay_reuses_original_key_and_skips_successful_items() {
    let mut l = SubmissionLedger::new();
    l.submit("C-1", "WO-1", true);
    l.submit("C-1", "WO-2", false);
    let replayed = l.replay("C-1");
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].key, idempotency_key("C-1", "WO-2"));
    assert_eq!(replayed[0].outcome, ReceiptOutcome::Success);
    assert_eq!(l.get_by_key(&idempotency_key("C-1", "WO-1")).unwrap().retry_count, 0);
    assert_eq!(l.active_count(), 2);
}
