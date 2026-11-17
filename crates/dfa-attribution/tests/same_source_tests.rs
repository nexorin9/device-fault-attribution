//! T25 proof：三值判定语义 / 证据不全判证据不足且不并簇 /
//! 缺证据字段即拒绝同源结论 / 候选集截断有记录。

use dfa_attribution::{EvidenceSet, SemanticComparator, SemanticVerdict};
use dfa_core::FaultVocabulary;

fn ev() -> EvidenceSet {
    EvidenceSet {
        model: "M-EXAMPLE".into(),
        firmware: "FW-EXAMPLE-1".into(),
        batch: "LOT-EXAMPLE-1".into(),
        time_window: "30d".into(),
        ward: "重症医学科".into(),
    }
}

#[test]
fn three_verdicts_are_distinguishable() {
    let c = SemanticComparator::default();
    let same = c.judge(&ev(), &ev(), "不制冷", "没冷气");
    assert_eq!(same.verdict, SemanticVerdict::Same);
    assert_eq!(same.considered, 2);
    assert!(same.evidence.iter().any(|e| e.starts_with("model=")));
    assert!(same.evidence.iter().any(|e| e.starts_with("firmware=")));
    assert!(same.evidence.iter().any(|e| e.starts_with("batch=")));
    assert!(same.evidence.iter().any(|e| e.starts_with("time_window=")));

    let different = c.judge(&ev(), &ev(), "不制冷", "一直报警");
    assert_eq!(different.verdict, SemanticVerdict::Different);

    let mut bad = ev();
    bad.batch = "  ".into();
    let insufficient = c.judge(&ev(), &bad, "不制冷", "不制冷");
    assert_eq!(insufficient.verdict, SemanticVerdict::Insufficient);
}

#[test]
fn missing_evidence_field_refuses_same_source_conclusion() {
    let mut a = ev();
    a.firmware = String::new();
    a.batch = String::new();
    let r = SemanticComparator::default().judge(&a, &ev(), "不制冷", "不制冷");
    assert_eq!(r.verdict, SemanticVerdict::Insufficient);
    assert!(r.missing_fields.contains(&"firmware".to_string()));
    assert!(r.missing_fields.contains(&"batch".to_string()));
    assert!(r.evidence.iter().any(|e| e.contains("不并簇")));
    assert!(r.missing_fields.contains(&"firmware".to_string()) || r.missing_fields.len() >= 2);
}

#[test]
fn candidate_group_truncation_is_recorded() {
    let c = SemanticComparator {
        candidate_limit: 2,
        vocabulary: FaultVocabulary::standard(),
    };
    let items = vec![
        ("K1".to_string(), ev(), "不制冷".to_string()),
        ("K2".to_string(), ev(), "没冷气".to_string()),
        ("K3".to_string(), ev(), "一直报警".to_string()),
    ];
    let r = c.compare_group(&items);
    assert!(r.truncated, "超上限必须留截断记录");
    assert_eq!(r.considered, 2);
}

#[test]
fn insufficient_candidate_group_does_not_force_a_cluster() {
    let c = SemanticComparator::default();
    let r = c.compare_group(&[("K1".to_string(), ev(), "不制冷".to_string())]);
    assert_eq!(r.verdict, SemanticVerdict::Insufficient);
    assert!(!r.truncated);
}
