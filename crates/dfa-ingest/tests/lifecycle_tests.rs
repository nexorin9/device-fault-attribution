//! T38 proof：非法跃迁被拒、退避间隔递增、休眠期间出站请求计数 = 0、
//! 唤醒后强制刷新计数 = 1。

use dfa_ingest::{Lifecycle, LifecycleState};

#[test]
fn illegal_transition_is_rejected_and_state_is_unchanged() {
    let mut lc = Lifecycle::new();
    assert_eq!(lc.state, LifecycleState::Booted);
    let err = lc.transition(LifecycleState::Running).unwrap_err();
    assert!(err.contains("非法跃迁"));
    assert_eq!(lc.state, LifecycleState::Booted, "返回原状态");
    assert!(!lc.is_legal_next(LifecycleState::Dormant));

    assert_eq!(
        lc.transition(LifecycleState::Wanted).unwrap(),
        LifecycleState::Wanted
    );
    assert_eq!(
        lc.transition(LifecycleState::Armed).unwrap(),
        LifecycleState::Armed
    );
    assert_eq!(
        lc.transition(LifecycleState::Running).unwrap(),
        LifecycleState::Running
    );
}

#[test]
fn poll_backoff_increases_exponentially_and_resets_on_success() {
    let mut lc = Lifecycle::new();
    let base = lc.base_poll_ms;
    let a = lc.poll_failure();
    let b = lc.poll_failure();
    let c = lc.poll_failure();
    assert!(a < b && b < c, "退避序列间隔递增");
    assert_eq!(a, base * 2);
    assert_eq!(b, base * 4);
    assert_eq!(c, base * 8);
    assert_eq!(lc.backoff_events, 3);
    assert_eq!(lc.consecutive_failures, 3);
    lc.poll_success();
    assert_eq!(lc.current_poll_ms, base);
    assert_eq!(lc.consecutive_failures, 0);
}

#[test]
fn sleep_blocks_outbound_io_and_wake_forces_exactly_one_refresh() {
    let mut lc = Lifecycle::new();
    lc.transition(LifecycleState::Wanted).unwrap();
    lc.transition(LifecycleState::Running).unwrap();
    lc.sleep();
    assert_eq!(lc.state, LifecycleState::Dormant);
    assert!(!lc.allow_outbound(), "休眠期间不得发起出站 I/O");
    assert_eq!(lc.outbound_during_sleep, 0, "休眠期间出站请求计数 = 0");
    lc.wake();
    assert_eq!(lc.state, LifecycleState::Running);
    assert_eq!(lc.wake_refresh_count, 1, "唤醒后强制刷新恰好 1 次");
    assert_eq!(lc.current_poll_ms, lc.base_poll_ms);
    lc.wake();
    assert_eq!(lc.wake_refresh_count, 1, "非休眠态再次唤醒不重复刷新");
}

#[test]
fn idle_returns_to_standby_and_interaction_resumes_service() {
    let mut lc = Lifecycle::new();
    lc.transition(LifecycleState::Wanted).unwrap();
    lc.transition(LifecycleState::Running).unwrap();
    lc.mark_idle();
    assert_eq!(lc.state, LifecycleState::Wanted);
    assert!(lc.allow_outbound(), "待命保留托盘与轮询");
    lc.interact();
    assert_eq!(lc.state, LifecycleState::Running);
    assert_eq!(LifecycleState::Dormant.label(), "休眠");
}
