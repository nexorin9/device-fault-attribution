use dfa_core::ClusterState;

#[test]
fn illegal_jump_to_ordered_is_rejected_and_returns_original() {
    let s = ClusterState::Observing;
    assert!(s.try_transition(ClusterState::Ordered, true).is_err());
    assert_eq!(
        s.transition_or_self(ClusterState::Ordered, true),
        ClusterState::Observing
    );
}

#[test]
fn human_confirmation_is_the_only_driver() {
    assert!(ClusterState::Observing
        .try_transition(ClusterState::ConfirmedSameSource, false)
        .is_err());
    assert_eq!(
        ClusterState::Observing
            .try_transition(ClusterState::ConfirmedSameSource, true)
            .unwrap(),
        ClusterState::ConfirmedSameSource
    );
}

#[test]
fn legal_path_is_adjacent_only() {
    assert!(ClusterState::ConfirmedSameSource.is_legal_next(ClusterState::Ordered));
    assert!(ClusterState::Ordered.is_legal_next(ClusterState::Closed));
    assert!(!ClusterState::ConfirmedSameSource.is_legal_next(ClusterState::Closed));
    assert_eq!(ClusterState::Closed.label(), "已闭环");
}
