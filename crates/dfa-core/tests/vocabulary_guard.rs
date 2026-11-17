use dfa_core::{FaultVocabulary, MergeSource, PENDING_MANUAL};

#[test]
fn out_of_vocabulary_is_guarded_to_pending_manual() {
    let v = FaultVocabulary::standard();
    let g = v.guard("MADE_UP_CATEGORY");
    assert!(!g.accepted);
    assert_eq!(g.category, PENDING_MANUAL);
    assert!(g.reason.contains("拒绝新建类别"));
}

#[test]
fn exact_key_wins_over_alias() {
    let v = FaultVocabulary::standard();
    let exact = v.merge("制冷失效");
    assert_eq!(exact.category, "REFUSAL_TO_COOL");
    assert_eq!(exact.source, MergeSource::Exact);
    let alias = v.merge("没冷气");
    assert_eq!(alias.category, "REFUSAL_TO_COOL");
    assert_eq!(alias.source, MergeSource::Alias);
}

#[test]
fn three_colloquial_forms_share_one_controlled_category() {
    let v = FaultVocabulary::standard();
    let ids: Vec<String> = ["不制冷", "没冷气", "制冷不给力"]
        .iter()
        .map(|t| v.merge(t).category)
        .collect();
    assert!(ids.iter().all(|c| c == "REFUSAL_TO_COOL"));
}

#[test]
fn no_new_category_entry_point_exists() {
    let v = FaultVocabulary::standard();
    let before = v.ids();
    let _ = v.guard("UNKNOWN_CAT");
    let _ = v.merge("完全没见过的描述");
    assert_eq!(v.ids(), before);
    assert_eq!(v.merge("完全没见过的描述").category, PENDING_MANUAL);
}
