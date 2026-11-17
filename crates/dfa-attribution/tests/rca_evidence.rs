//! T26 proof：编造证据字段被拒且落审计 / 合法假设保持 draft /
//! 置信档位与假设数量上限生效。

use dfa_attribution::{Confidence, EvidenceValues, Hypothesis, RcaValidator, MAX_HYPOTHESES};

fn h(text: &str, refs: &[&str], confidence: Confidence) -> Hypothesis {
    Hypothesis {
        text: text.into(),
        evidence_refs: refs.iter().map(|s| s.to_string()).collect(),
        confidence,
        falsifiable_rank: 1,
        draft: false,
    }
}

fn evidence() -> EvidenceValues {
    EvidenceValues::from_strings(&[
        "M-EXAMPLE".into(),
        "FW-EXAMPLE-1".into(),
        "LOT-EXAMPLE-1".into(),
        "重症医学科".into(),
        "30d".into(),
    ])
}

#[test]
fn fabricated_reference_is_rejected_with_auditable_reason() {
    let r = RcaValidator::default().validate_all(
        &[h("批次固件不匹配", &["FW-FAKE-000"], Confidence::High)],
        &evidence(),
    );
    assert!(r.accepted.is_empty());
    assert_eq!(r.rejected.len(), 1);
    assert!(r.rejected[0].1.contains("编造证据引用"));
    assert!(r.rejected[0].0.contains("批次固件不匹配"), "被拒项需带原文供审计回查");
}

#[test]
fn valid_hypothesis_stays_draft_and_confidence_labels_are_distinct() {
    let r = RcaValidator::default().validate_all(
        &[
            h("制冷回路泄漏", &["M-EXAMPLE"], Confidence::High),
            h("固件版本回退", &["FW-EXAMPLE-1"], Confidence::Medium),
            h("批次批次差异", &["LOT-EXAMPLE-1"], Confidence::Low),
        ],
        &evidence(),
    );
    assert_eq!(r.accepted.len(), 3);
    assert!(r.accepted.iter().all(|x| x.draft), "未确认前任何路径都不得转 confirmed");
    assert_eq!(Confidence::High.label(), "高");
    assert_eq!(Confidence::Medium.label(), "中");
    assert_eq!(Confidence::Low.label(), "低");
}

#[test]
fn hypothesis_count_limit_is_enforced() {
    let mut all = Vec::new();
    for i in 0..MAX_HYPOTHESES + 2 {
        all.push(h(&format!("假设-{i}"), &["M-EXAMPLE"], Confidence::Low));
    }
    let r = RcaValidator::default().validate_all(&all, &evidence());
    assert_eq!(r.accepted.len(), MAX_HYPOTHESES);
    assert_eq!(r.rejected.len(), 2);
    assert!(r.rejected.iter().all(|(_, why)| why == "超出假设数量上限"));
}

#[test]
fn empty_text_or_empty_refs_are_rejected_separately() {
    let v = RcaValidator::default();
    assert!(v.validate_one(&h("", &["M-EXAMPLE"], Confidence::Low), &evidence()).unwrap_err().contains("为空"));
    assert!(v
        .validate_one(&h("有文本", &[], Confidence::Low), &evidence())
        .unwrap_err()
        .contains("未引用任何簇内证据字段"));
}
