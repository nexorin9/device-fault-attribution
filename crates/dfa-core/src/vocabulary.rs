//! 受控故障类别词表与归并守卫。
//! 词表以稳定标识符为键，**接口层不提供新增类别的入口**；词表外一律
//! 「待人工归类」，不静默新建类别。精确键优先、别名回退。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PENDING_MANUAL: &str = "PENDING_MANUAL";
pub const DEGRADE_LABEL: &str = "规则判定 · 待人工判断";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeSource {
    Exact,
    Alias,
    Model,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeOutcome {
    pub category: String,
    pub evidence: String,
    pub source: MergeSource,
    pub in_vocabulary: bool,
    /// 词表外归并被守卫拒绝时为 true，供审计链留痕。
    pub rejected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardResult {
    pub accepted: bool,
    pub category: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultVocabulary {
    /// 受控类别标识符 → 该类别的规范术语（精确匹配用）。
    pub categories: BTreeMap<String, Vec<String>>,
    /// 科室口语片段 / 厂商术语 → 受控类别标识符。
    pub aliases: BTreeMap<String, String>,
}

impl FaultVocabulary {
    /// 出厂受控词表。重命名标识符属于迁移事件，由 dfa-config 校验检出。
    pub fn standard() -> Self {
        let mut categories = BTreeMap::new();
        categories.insert(
            "REFUSAL_TO_COOL".to_string(),
            vec!["制冷失效".to_string(), "不制冷".to_string()],
        );
        categories.insert(
            "NO_RESPONSE".to_string(),
            vec!["无响应".to_string(), "开机无反应".to_string()],
        );
        categories.insert(
            "ALARM_ONLY".to_string(),
            vec!["持续报警".to_string(), "误报警".to_string()],
        );
        categories.insert(
            "INFUSION_BLOCKED".to_string(),
            vec!["输液受阻".to_string(), "管路堵塞".to_string()],
        );
        categories.insert(
            "POWER_FAILURE".to_string(),
            vec!["电源故障".to_string(), "无法开机".to_string()],
        );
        categories.insert(
            "DISPLAY_ABNORMAL".to_string(),
            vec!["显示异常".to_string(), "花屏".to_string()],
        );

        let mut aliases = BTreeMap::new();
        for (phrase, id) in [
            ("不制冷", "REFUSAL_TO_COOL"),
            ("不降温", "REFUSAL_TO_COOL"),
            ("没冷气", "REFUSAL_TO_COOL"),
            ("制冷不给力", "REFUSAL_TO_COOL"),
            ("没反应", "NO_RESPONSE"),
            ("黑屏不启动", "NO_RESPONSE"),
            ("按了没动静", "NO_RESPONSE"),
            ("只会滴滴响", "ALARM_ONLY"),
            ("一直报警", "ALARM_ONLY"),
            ("滴滴响", "ALARM_ONLY"),
            ("输不进去", "INFUSION_BLOCKED"),
            ("推不动药", "INFUSION_BLOCKED"),
            ("掉电", "POWER_FAILURE"),
            ("通电没反应", "POWER_FAILURE"),
            ("屏幕闪烁", "DISPLAY_ABNORMAL"),
            ("屏幕花了", "DISPLAY_ABNORMAL"),
        ] {
            aliases.insert(phrase.to_string(), id.to_string());
        }
        FaultVocabulary { categories, aliases }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.categories.contains_key(id)
    }

    pub fn ids(&self) -> Vec<String> {
        self.categories.keys().cloned().collect()
    }

    /// 词表守卫：类别必须落在受控标识符集合内；词表外一律待人工归类。
    pub fn guard(&self, category: &str) -> GuardResult {
        if self.contains(category) {
            GuardResult {
                accepted: true,
                category: category.to_string(),
                reason: "命中受控类别".to_string(),
            }
        } else {
            GuardResult {
                accepted: false,
                category: PENDING_MANUAL.to_string(),
                reason: format!("词表外类别 {category} → 待人工归类，拒绝新建类别"),
            }
        }
    }

    /// 确定性归并：受控词表精确 → 别名表回退 → 词表外归「待人工归类」。
    pub fn merge(&self, text: &str) -> MergeOutcome {
        let trimmed = text.trim();
        for (id, terms) in &self.categories {
            for term in terms {
                if trimmed == term || trimmed.contains(term) {
                    return MergeOutcome {
                        category: id.clone(),
                        evidence: format!("词表精确键 {id}:{term}"),
                        source: MergeSource::Exact,
                        in_vocabulary: true,
                        rejected: false,
                    };
                }
            }
        }
        for (phrase, id) in &self.aliases {
            if trimmed.contains(phrase) {
                return MergeOutcome {
                    category: id.clone(),
                    evidence: format!("别名键 {phrase}→{id}"),
                    source: MergeSource::Alias,
                    in_vocabulary: true,
                    rejected: false,
                };
            }
        }
        MergeOutcome {
            category: PENDING_MANUAL.to_string(),
            evidence: "无命中：精确键与别名表均未命中".to_string(),
            source: MergeSource::None,
            in_vocabulary: false,
            rejected: false,
        }
    }
}
