//! T9 proof：3 种口语说法归并为同一受控类别 / 词表外不新增类别 /
//! 证据不足判证据不足且不并簇 / 模型不可达输出带降级标记且簇状态未推进。

use dfa_attribution::{
    rules_merge, EvidenceSet, MergeChain, SemanticComparator, SemanticMerger, SemanticVerdict,
};
use dfa_core::{ClusterState, FaultVocabulary, DEGRADE_LABEL, PENDING_MANUAL};

struct FixedMerger {
    result: Result<Option<String>, String>,
}

impl SemanticMerger for FixedMerger {
    fn merge(&self, _text: &str, _candidates: &[String]) -> Result<Option<String>, String> {
        self.result.clone()
    }
}

fn evidence() -> EvidenceSet {
    EvidenceSet {
        model: "M-EXAMPLE".into(),
        firmware: "FW-EXAMPLE-1".into(),
        batch: "LOT-EXAMPLE-1".into(),
        time_window: "30d".into(),
        ward: "设备科".into(),
    }
}

#[test]
fn three_colloquial_descriptions_merge_to_one_controlled_category() {
    let outcomes = ["不制冷", "没冷气", "制冷不给力"]
        .iter()
        .map(|t| rules_merge(t))
        .collect::<Vec<_>>();
    for o in &outcomes {
        assert_eq!(o.outcome.category, "REFUSAL_TO_COOL");
        assert!(o.outcome.in_vocabulary);
        assert!(!o.outcome.evidence.is_empty(), "归并结果必须带命中证据");
    }
    assert_eq!(outcomes[0].outcome.source, dfa_core::MergeSource::Exact);
    assert_eq!(outcomes[1].outcome.source, dfa_core::MergeSource::Alias);
}

#[test]
fn out_of_vocabulary_description_never_creates_a_new_category() {
    let before = FaultVocabulary::standard().ids();
    let result = rules_merge("设备外壳轻微划痕，功能未见异常");
    assert_eq!(result.outcome.category, PENDING_MANUAL);
    assert!(!result.guard.accepted);
    assert_eq!(result.label, "待人工归类 · 不新增类别");
    assert_eq!(FaultVocabulary::standard().ids(), before, "词表标识符集合不得变化");

    let model = MergeChain::with_model(Box::new(FixedMerger {
        result: Ok(Some("BRAND_NEW_CATEGORY".into())),
    }));
    let rejected = model.merge("某新型异常表现");
    assert_eq!(rejected.outcome.category, PENDING_MANUAL);
    assert!(!rejected.rejected_audit.is_empty(), "被拒的模型输出必须留审计痕迹");
    assert!(rejected.rejected_audit[0].contains("拒绝写入新类别"));
}

#[test]
fn insufficient_evidence_is_insufficient_and_never_merges_cluster() {
    let mut a = evidence();
    a.firmware = String::new();
    let r = SemanticComparator::default().judge(&a, &evidence(), "不制冷", "不制冷");
    assert_eq!(r.verdict, SemanticVerdict::Insufficient);
    assert!(r.missing_fields.contains(&"firmware".to_string()));
    assert!(r.evidence.iter().any(|e| e.contains("不并簇")));
}

#[test]
fn model_unreachable_is_labelled_degraded_and_state_is_not_advanced() {
    let chain = MergeChain::with_model(Box::new(FixedMerger {
        result: Err("ERR_MODEL_UNREACHABLE: 超时".into()),
    }));
    let r = chain.merge("压缩机像是不干活了");
    assert!(r.degraded);
    assert_eq!(r.label, DEGRADE_LABEL);
    assert_eq!(r.outcome.category, PENDING_MANUAL);

    let state = ClusterState::Observing;
    let after = state.transition_or_self(ClusterState::ConfirmedSameSource, false);
    assert_eq!(after, ClusterState::Observing, "降级路径不得推进簇状态");
    assert!(state
        .try_transition(ClusterState::Ordered, true)
        .unwrap_err()
        .reason
        .contains("ERR_ILLEGAL_TRANSITION"));
}
