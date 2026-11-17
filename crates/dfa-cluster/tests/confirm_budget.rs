use dfa_cluster::{carry, BudgetLedger, ConfirmBudget, TriggerKind};
use dfa_core::ClusterState;

#[test]
fn carry_is_max_of_new_budget_and_decremented_carry() {
    let prev = ConfirmBudget::initial(TriggerKind::NewCluster, 4, 1);
    assert_eq!(prev.remaining, 4);
    let next = carry(&prev, "K-1", 1);
    assert_eq!(next.remaining, 3);
    assert_eq!(next.carried_from_key, "K-1");
    let extended = carry(&prev, "K-1", 6);
    assert_eq!(extended.remaining, 6);
}

#[test]
fn multi_tick_scenario_never_shrinks_budget() {
    let mut ledger = BudgetLedger::new();
    let b1 = ledger.on_trigger("K", TriggerKind::NewCluster, 4, 1);
    assert_eq!(b1.remaining, 4);
    let b2 = ledger.on_trigger("K", TriggerKind::NewCluster, 4, 1);
    assert_eq!(b2.remaining, 4);
    let b3 = ledger.on_trigger("K", TriggerKind::Recurring, 4, 1);
    assert_eq!(b3.remaining, 3);
    assert!(b3.remaining >= b2.remaining - 1);
}

#[test]
fn five_tick_sequence_keeps_max_invariant() {
    let mut ledger = BudgetLedger::new();
    let mut prev = ledger.on_trigger("K", TriggerKind::NewCluster, 4, 1);
    for _ in 0..5 {
        let new_budget = 2;
        let next = carry(&prev, "K", new_budget);
        assert_eq!(next.remaining, new_budget.max(prev.remaining.saturating_sub(1)));
        prev = next;
    }
}

#[test]
fn exhausted_budget_keeps_observing_state_and_zero_outbound() {
    let mut ledger = BudgetLedger::new();
    ledger.on_trigger("K", TriggerKind::Recurring, 4, 1);
    for _ in 0..5 {
        ledger.tick("K");
    }
    assert!(ledger.exhausted("K"));
    assert_eq!(ledger.outbound_writes, 0);
    assert!(!ledger.exhausted_notes.is_empty());
    assert!(!ledger.confirm("K"));
    assert_eq!(
        ClusterState::Observing.transition_or_self(ClusterState::Ordered, false),
        ClusterState::Observing
    );
}
