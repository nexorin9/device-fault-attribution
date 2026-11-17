//! T10 proof：编造证据字段的假设被拒且有审计记录 / 能力表外处置项被剔除 /
//! 未确认时全部产出 draft / 超出同源时间窗不出同源结论。

use dfa_attribution::{
    confirm, order_steps, time_window_expired, Confidence, ConfirmRequest, EvidenceValues,
    Hypothesis, RcaValidator, Step,
};
use dfa_core::{Cluster, ClusterState, TeamCapability};

fn teams() -> Vec<TeamCapability> {
    vec![TeamCapability {
        team: "设备维修一组".into(),
        capabilities: vec!["检查制冷回路".into(), "核查固件版本".into()],
    }]
}

fn hypothesis(refs: &[&str]) -> Hypothesis {
    Hypothesis {
        text: "制冷回路泄漏导致不制冷".into(),
        evidence_refs: refs.iter().map(|s| s.to_string()).collect(),
        confidence: Confidence::Medium,
        falsifiable_rank: 1,
        draft: false,
    }
}

fn step(action: &str, capability: &str, rank: u32, critical: bool) -> Step {
    Step {
        action: action.into(),
        expected: "观察压力表读数".into(),
        criterion: "读数低于阈值即证伪".into(),
        reason: "最可能证伪".into(),
        falsifiable_rank: rank,
        critical_ward: critical,
        capability: capability.into(),
    }
}

fn confirm_request() -> ConfirmRequest {
    ConfirmRequest {
        category: "REFUSAL_TO_COOL".into(),
        members_verified: true,
        team: "设备维修一组".into(),
        capabilities: teams(),
        earliest_epoch: 1_000,
        now_epoch: 1_500,
        window_secs: 30 * 24 * 3600,
        actor: "设备科值班工程师".into(),
        ts: 1_600,
        member_snapshot: vec!["DEV-EXAMPLE-01".into()],
    }
}

#[test]
fn fabricated_evidence_reference_is_rejected_with_audit_record() {
    let evidence = EvidenceValues::from_strings(&[
        "M-EXAMPLE".into(),
        "FW-EXAMPLE-1".into(),
        "LOT-EXAMPLE-1".into(),
    ]);
    let v = RcaValidator::default();
    let result = v.validate_all(&[hypothesis(&["M-EXAMPLE", "FW-FABRICATED-999"])], &evidence);
    assert!(result.accepted.is_empty());
    assert_eq!(result.rejected.len(), 1, "被拒项必须留痕供审计链使用");
    assert!(result.rejected[0].1.contains("编造证据引用"));
}

#[test]
fn step_outside_capability_table_is_removed_with_reason() {
    let ordered = order_steps(
        vec![
            step("检查制冷回路", "检查制冷回路", 1, false),
            step("返厂更换主板", "返厂更换主板", 0, false),
        ],
        &teams(),
    );
    assert_eq!(ordered.actions(), vec!["检查制冷回路".to_string()]);
    assert_eq!(ordered.removed.len(), 1);
    assert!(ordered.removed[0].1.contains("不在当次班组能力表内"));
}

#[test]
fn all_outputs_stay_draft_before_human_confirmation() {
    let evidence = EvidenceValues::from_strings(&["M-EXAMPLE".into()]);
    let accepted = RcaValidator::default().validate_all(&[hypothesis(&["M-EXAMPLE"])], &evidence);
    assert_eq!(accepted.accepted.len(), 1);
    assert!(accepted.accepted[0].draft, "模型产出恒为草稿");
    let cluster = Cluster::observing("C-EXAMPLE-1", "M-EXAMPLE|FW|LOT", "REFUSAL_TO_COOL");
    assert!(cluster.draft);
    assert_eq!(cluster.state, ClusterState::Observing);
    // 未确认的状态机跃迁被拒，因而不会出现 confirmed 产出。
    assert!(ClusterState::Observing
        .try_transition(ClusterState::ConfirmedSameSource, false)
        .is_err());
}

#[test]
fn expired_time_window_does_not_produce_same_source_conclusion() {
    let mut req = confirm_request();
    req.now_epoch = req.earliest_epoch + req.window_secs + 1;
    assert!(time_window_expired(&req));
    let err = confirm(&req, ClusterState::Observing).unwrap_err();
    assert!(
        err.iter().any(|m| m.contains("需重新观察") && m.contains("不生成工单")),
        "超窗必须降级为重新观察，不出同源结论、不生成工单：{err:?}"
    );
}
