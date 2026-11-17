//! T28 proof：未确认时全部产出 draft / 超时间窗不出同源结论 /
//! 缺项确认被拒并列出缺项 / 确认记录含操作人与成员快照。

use dfa_attribution::{confirm, time_window_expired, ConfirmRequest};
use dfa_core::{Cluster, ClusterState, TeamCapability};

fn teams() -> Vec<TeamCapability> {
    vec![TeamCapability {
        team: "设备维修一组".into(),
        capabilities: vec!["检查制冷回路".into()],
    }]
}

fn req() -> ConfirmRequest {
    ConfirmRequest {
        category: "REFUSAL_TO_COOL".into(),
        members_verified: true,
        team: "设备维修一组".into(),
        capabilities: teams(),
        earliest_epoch: 10_000,
        now_epoch: 10_100,
        window_secs: 30 * 24 * 3600,
        actor: "设备科值班工程师".into(),
        ts: 10_200,
        member_snapshot: vec!["DEV-EXAMPLE-01".into(), "DEV-EXAMPLE-02".into()],
    }
}

#[test]
fn outputs_are_draft_until_a_human_confirms() {
    let c = Cluster::observing("C-1", "M|FW|LOT", "REFUSAL_TO_COOL");
    assert!(c.draft);
    assert_eq!(c.state, ClusterState::Observing);
    assert!(ClusterState::Observing
        .try_transition(ClusterState::Ordered, true)
        .is_err());
    let ok = confirm(&req(), c.state).unwrap();
    assert!(!ok.draft, "确认动作本身不是草稿");
    assert_eq!(ok.after_state, "已确认同源");
}

#[test]
fn expired_window_produces_no_same_source_conclusion() {
    let mut r = req();
    r.now_epoch = r.earliest_epoch + r.window_secs + 1;
    assert!(time_window_expired(&r));
    let err = confirm(&r, ClusterState::Observing).unwrap_err();
    assert!(err.iter().any(|m| m.contains("不出同源结论")));
}

#[test]
fn missing_preconditions_are_rejected_with_a_complete_list() {
    let mut r = req();
    r.category = "NOT_IN_VOCABULARY".into();
    r.members_verified = false;
    r.team = "未登记班组".into();
    let err = confirm(&r, ClusterState::Observing).unwrap_err();
    assert_eq!(err.len(), 3, "三项缺项必须一次列全");
    assert!(err.iter().any(|m| m.contains("受控词表")));
    assert!(err.iter().any(|m| m.contains("未核实设备")));
    assert!(err.iter().any(|m| m.contains("能力表")));
}

#[test]
fn confirmation_record_carries_actor_and_member_snapshot() {
    let r = req();
    let ok = confirm(&r, ClusterState::Observing).unwrap();
    assert_eq!(ok.actor, "设备科值班工程师");
    assert_eq!(ok.ts, 10_200);
    assert_eq!(ok.member_snapshot, r.member_snapshot, "确认时簇内有哪些设备可回查");
    assert_eq!(ok.before_state, "观察中");
    assert_eq!(ok.after_state, "已确认同源");
    assert_eq!(ok.items.len(), 2);
}
