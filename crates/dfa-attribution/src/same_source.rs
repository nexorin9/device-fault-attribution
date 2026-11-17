//! 跨工单同源语义判定：在硬约束已过滤的候选集内做语义比较（不是字段相等），
//! 输出三值（同源 / 不同源 / 证据不足），判定结果必须携带真实证据字段。

use dfa_core::{FaultVocabulary, PENDING_MANUAL};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const DEFAULT_CANDIDATE_LIMIT: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticVerdict {
    Same,
    Different,
    Insufficient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceSet {
    pub model: String,
    pub firmware: String,
    pub batch: String,
    pub time_window: String,
    pub ward: String,
}

impl EvidenceSet {
    pub fn missing(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.model.trim().is_empty() {
            out.push("model".to_string());
        }
        if self.firmware.trim().is_empty() {
            out.push("firmware".to_string());
        }
        if self.batch.trim().is_empty() {
            out.push("batch".to_string());
        }
        if self.time_window.trim().is_empty() {
            out.push("time_window".to_string());
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompareResult {
    pub verdict: SemanticVerdict,
    pub evidence: Vec<String>,
    pub missing_fields: Vec<String>,
    pub truncated: bool,
    pub considered: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticComparator {
    pub candidate_limit: usize,
    pub vocabulary: FaultVocabulary,
}

impl Default for SemanticComparator {
    fn default() -> Self {
        SemanticComparator {
            candidate_limit: DEFAULT_CANDIDATE_LIMIT,
            vocabulary: FaultVocabulary::standard(),
        }
    }
}

impl SemanticComparator {
    /// 单对比较：证据字段缺任一即判「证据不足」并拒绝给出同源结论。
    pub fn judge(&self, a: &EvidenceSet, b: &EvidenceSet, text_a: &str, text_b: &str) -> CompareResult {
        let mut missing = a.missing();
        for m in b.missing() {
            if !missing.contains(&m) {
                missing.push(m);
            }
        }
        if !missing.is_empty() {
            return CompareResult {
                verdict: SemanticVerdict::Insufficient,
                evidence: vec!["证据不足：不并簇，输出待补录字段清单".to_string()],
                missing_fields: missing,
                truncated: false,
                considered: 2,
            };
        }
        let ca = self.vocabulary.merge(text_a);
        let cb = self.vocabulary.merge(text_b);
        if ca.category == PENDING_MANUAL || cb.category == PENDING_MANUAL {
            return CompareResult {
                verdict: SemanticVerdict::Insufficient,
                evidence: vec!["故障描述待人工归类，无法给出同源结论".to_string()],
                missing_fields: vec!["fault_category".to_string()],
                truncated: false,
                considered: 2,
            };
        }
        // 语义比较以归并后的受控类别为准，不以原始字段相等为依据。
        let verdict = if ca.category == cb.category {
            SemanticVerdict::Same
        } else {
            SemanticVerdict::Different
        };
        CompareResult {
            verdict,
            evidence: vec![
                format!("model={}", a.model),
                format!("firmware={}", a.firmware),
                format!("batch={}", a.batch),
                format!("time_window={}", a.time_window),
                format!("category_a={} ({})", ca.category, ca.evidence),
                format!("category_b={} ({})", cb.category, cb.evidence),
            ],
            missing_fields: Vec::new(),
            truncated: false,
            considered: 2,
        }
    }

    /// 候选集规模控制：按 cluster_key 分组后比较，超过上限即截断并留记录。
    pub fn compare_group(
        &self,
        items: &[(String, EvidenceSet, String)],
    ) -> CompareResult {
        let mut considered = items.len();
        let mut truncated = false;
        let slice = if items.len() > self.candidate_limit {
            considered = self.candidate_limit;
            truncated = true;
            &items[..self.candidate_limit]
        } else {
            items
        };
        if slice.len() < 2 {
            return CompareResult {
                verdict: SemanticVerdict::Insufficient,
                evidence: vec!["候选集不足两例".to_string()],
                missing_fields: Vec::new(),
                truncated,
                considered,
            };
        }
        let base = &slice[0];
        let mut all_same = true;
        let mut evidence = Vec::new();
        for other in &slice[1..] {
            let r = self.judge(&base.1, &other.1, &base.2, &other.2);
            if r.verdict != SemanticVerdict::Same {
                all_same = false;
            }
            evidence.extend(r.evidence);
            if r.verdict == SemanticVerdict::Insufficient {
                return CompareResult {
                    verdict: SemanticVerdict::Insufficient,
                    evidence,
                    missing_fields: r.missing_fields,
                    truncated,
                    considered,
                };
            }
        }
        // 去重后保留证据字段，便于审计链回查。
        let mut seen = HashSet::new();
        evidence.retain(|e| seen.insert(e.clone()));
        CompareResult {
            verdict: if all_same {
                SemanticVerdict::Same
            } else {
                SemanticVerdict::Different
            },
            evidence,
            missing_fields: Vec::new(),
            truncated,
            considered,
        }
    }
}
