//! 描述归并三级链路：受控词表精确 → 别名表回退 → 模型语义同一化。
//! 模型只是第三级回退；归并结果不在词表内即「待人工归类」，拒绝写入新类别。

use dfa_core::{
    FaultVocabulary, GuardResult, MergeOutcome, MergeSource, DEGRADE_LABEL, PENDING_MANUAL,
};
use serde::{Deserialize, Serialize};

/// 模型语义同一化接口。Err 表示模型不可达（走确定性回退），
/// Ok(None) 表示模型未给出可用归并结果。
pub trait SemanticMerger {
    fn merge(&self, text: &str, candidates: &[String]) -> Result<Option<String>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeChainResult {
    pub outcome: MergeOutcome,
    pub guard: GuardResult,
    /// 被拒绝的模型输出（词表外）落审计链的理由清单。
    pub rejected_audit: Vec<String>,
    pub degraded: bool,
    pub label: String,
}

pub struct MergeChain {
    pub vocabulary: FaultVocabulary,
    pub merger: Option<Box<dyn SemanticMerger>>,
}

impl Default for MergeChain {
    fn default() -> Self {
        MergeChain {
            vocabulary: FaultVocabulary::standard(),
            merger: None,
        }
    }
}

impl MergeChain {
    pub fn rules_only() -> Self {
        MergeChain::default()
    }

    pub fn with_model(merger: Box<dyn SemanticMerger>) -> Self {
        MergeChain {
            vocabulary: FaultVocabulary::standard(),
            merger: Some(merger),
        }
    }

    pub fn merge(&self, text: &str) -> MergeChainResult {
        let rule = self.vocabulary.merge(text);
        if rule.in_vocabulary {
            return MergeChainResult {
                guard: self.vocabulary.guard(&rule.category),
                outcome: rule,
                rejected_audit: Vec::new(),
                degraded: false,
                label: "规则命中 · 待人工确认".to_string(),
            };
        }

        // 第三级：模型语义同一化（科室口语 ↔ 厂商术语）。
        let mut degraded = false;
        let mut label = "模型草稿 · 待人工确认".to_string();
        let mut rejected_audit = Vec::new();
        let mut model_category: Option<String> = None;
        if let Some(merger) = &self.merger {
            match merger.merge(text, &self.vocabulary.ids()) {
                Ok(Some(id)) => model_category = Some(id),
                Ok(None) => {}
                Err(_) => {
                    degraded = true;
                    label = DEGRADE_LABEL.to_string();
                }
            }
        }

        if let Some(id) = model_category {
            let guard = self.vocabulary.guard(&id);
            if !guard.accepted {
                rejected_audit.push(format!(
                    "模型返回词表外类别 {id}：标「待人工归类」，拒绝写入新类别"
                ));
            } else {
                let outcome = MergeOutcome {
                    category: id.clone(),
                    evidence: format!("模型语义同一化 → 受控类别 {id}"),
                    source: MergeSource::Model,
                    in_vocabulary: true,
                    rejected: false,
                };
                return MergeChainResult {
                    outcome,
                    guard,
                    rejected_audit,
                    degraded,
                    label,
                };
            }
        }

        if degraded {
            label = DEGRADE_LABEL.to_string();
        } else {
            label = "待人工归类 · 不新增类别".to_string();
        }
        MergeChainResult {
            outcome: MergeOutcome {
                category: PENDING_MANUAL.to_string(),
                evidence: rule.evidence.clone(),
                source: if degraded { MergeSource::None } else { rule.source },
                in_vocabulary: false,
                rejected: false,
            },
            guard: GuardResult {
                accepted: false,
                category: PENDING_MANUAL.to_string(),
                reason: "词表外描述 → 待人工归类".to_string(),
            },
            rejected_audit,
            degraded,
            label,
        }
    }
}

/// 规则路径（无模型）：与产品主路径共用同一接口，只是不启用第三级。
pub fn rules_merge(text: &str) -> MergeChainResult {
    MergeChain::rules_only().merge(text)
}
