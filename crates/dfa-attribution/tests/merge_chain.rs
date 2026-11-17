//! T24 proof：三级链路（词表 → 别名 → 模型同一化）带命中证据 /
//! 词表外不新增类别且有被拒审计 / 模型不可达带降级标记。

use dfa_attribution::{MergeChain, SemanticMerger};
use dfa_core::{MergeSource, DEGRADE_LABEL, PENDING_MANUAL};

struct Merger {
    result: Result<Option<String>, String>,
}

impl SemanticMerger for Merger {
    fn merge(&self, _text: &str, _candidates: &[String]) -> Result<Option<String>, String> {
        self.result.clone()
    }
}

#[test]
fn three_levels_each_carry_hit_evidence() {
    let exact = MergeChain::rules_only().merge("制冷失效");
    assert_eq!(exact.outcome.category, "REFUSAL_TO_COOL");
    assert_eq!(exact.outcome.source, MergeSource::Exact);
    assert!(exact.outcome.evidence.contains("词表精确键"));

    let alias = MergeChain::rules_only().merge("没冷气");
    assert_eq!(alias.outcome.category, "REFUSAL_TO_COOL");
    assert_eq!(alias.outcome.source, MergeSource::Alias);
    assert!(alias.outcome.evidence.contains("别名键"));

    let model = MergeChain::with_model(Box::new(Merger {
        result: Ok(Some("REFUSAL_TO_COOL".into())),
    }));
    let semantic = model.merge("压缩机像是不干活了");
    assert_eq!(semantic.outcome.category, "REFUSAL_TO_COOL");
    assert_eq!(semantic.outcome.source, MergeSource::Model);
    assert!(semantic.outcome.evidence.contains("模型语义同一化"));
    assert!(semantic.guard.accepted);
}

#[test]
fn model_returning_out_of_vocabulary_is_rejected_and_audited() {
    let chain = MergeChain::with_model(Box::new(Merger {
        result: Ok(Some("TOTALLY_NEW_FAULT".into())),
    }));
    let r = chain.merge("压缩机像是不干活了");
    assert_eq!(r.outcome.category, PENDING_MANUAL);
    assert!(!r.guard.accepted);
    assert_eq!(r.rejected_audit.len(), 1);
    assert!(r.rejected_audit[0].contains("拒绝写入新类别"));
}

#[test]
fn model_unreachable_falls_back_to_rules_with_degrade_label() {
    let chain = MergeChain::with_model(Box::new(Merger {
        result: Err("ERR_MODEL_UNREACHABLE: 5xx".into()),
    }));
    let r = chain.merge("压缩机像是不干活了");
    assert!(r.degraded);
    assert_eq!(r.label, DEGRADE_LABEL);
    assert_eq!(r.outcome.source, MergeSource::None);
    assert_eq!(r.outcome.category, PENDING_MANUAL);
}
