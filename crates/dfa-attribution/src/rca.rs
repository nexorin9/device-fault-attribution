//! 根因假设生成与证据引用校验器：候选空间无法穷举为规则，只能由模型生成；
//! 但每条假设必须引用簇内真实存在的证据字段，编造即被拒并落审计。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_HYPOTHESES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn label(&self) -> &'static str {
        match self {
            Confidence::High => "高",
            Confidence::Medium => "中",
            Confidence::Low => "低",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub text: String,
    pub evidence_refs: Vec<String>,
    pub confidence: Confidence,
    pub falsifiable_rank: u32,
    pub draft: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EvidenceValues {
    pub values: HashSet<String>,
}

impl EvidenceValues {
    pub fn from_strings(items: &[String]) -> Self {
        EvidenceValues {
            values: items.iter().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RcaValidation {
    pub accepted: Vec<Hypothesis>,
    pub rejected: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RcaValidator {
    pub max_hypotheses: usize,
}

impl Default for RcaValidator {
    fn default() -> Self {
        RcaValidator {
            max_hypotheses: MAX_HYPOTHESES,
        }
    }
}

impl RcaValidator {
    pub fn validate_one(
        &self,
        h: &Hypothesis,
        evidence: &EvidenceValues,
    ) -> Result<(), String> {
        if h.text.trim().is_empty() {
            return Err("假设文本为空".to_string());
        }
        if h.evidence_refs.is_empty() {
            return Err("未引用任何簇内证据字段".to_string());
        }
        for r in &h.evidence_refs {
            if !evidence.values.contains(r) {
                return Err(format!("编造证据引用：{r} 不在簇内证据集合中"));
            }
        }
        Ok(())
    }

    /// 逐条核对；被拒项与原因随结果返回，供审计链留痕。数量上限生效。
    pub fn validate_all(
        &self,
        hypotheses: &[Hypothesis],
        evidence: &EvidenceValues,
    ) -> RcaValidation {
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        for h in hypotheses.iter().take(self.max_hypotheses) {
            match self.validate_one(h, evidence) {
                Ok(()) => {
                    let mut h = h.clone();
                    h.draft = true;
                    accepted.push(h);
                }
                Err(reason) => rejected.push((h.text.clone(), reason)),
            }
        }
        for h in hypotheses.iter().skip(self.max_hypotheses) {
            rejected.push((h.text.clone(), "超出假设数量上限".to_string()));
        }
        RcaValidation { accepted, rejected }
    }
}
