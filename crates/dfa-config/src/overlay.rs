//! 三层叠加（默认 → 按科室 → 按班组）与精确键优先、别名回退匹配器。
//! 与源产品不同的取舍：某层缺项时**显式缺省（标「未配置」）**，不回填下一层默认值。

use serde::{Deserialize, Serialize};

use crate::loader::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayLayer {
    Default,
    Ward,
    Team,
    Unconfigured,
}

impl OverlayLayer {
    pub fn label(&self) -> &'static str {
        match self {
            OverlayLayer::Default => "默认层",
            OverlayLayer::Ward => "按科室覆盖",
            OverlayLayer::Team => "按班组覆盖",
            OverlayLayer::Unconfigured => "未配置",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayResult {
    pub category: Option<String>,
    pub layer: OverlayLayer,
    pub note: String,
}

/// 按最具体层向默认层查找；三层都无配置时返回显式缺省。
pub fn resolve_category(config: &Config, ward: &str, team: &str) -> OverlayResult {
    let team_key = format!("team.{team}");
    let ward_key = format!("ward.{ward}");
    if let Some(v) = config.overrides.get(&team_key) {
        return OverlayResult {
            category: Some(v.clone()),
            layer: OverlayLayer::Team,
            note: format!("命中 {team_key}"),
        };
    }
    if let Some(v) = config.overrides.get(&ward_key) {
        return OverlayResult {
            category: Some(v.clone()),
            layer: OverlayLayer::Ward,
            note: format!("命中 {ward_key}"),
        };
    }
    if let Some(v) = config.overrides.get("default.category") {
        return OverlayResult {
            category: Some(v.clone()),
            layer: OverlayLayer::Default,
            note: "命中默认类别映射".to_string(),
        };
    }
    OverlayResult {
        category: None,
        layer: OverlayLayer::Unconfigured,
        note: "未配置：不回填默认值，标「待人工归类」".to_string(),
    }
}

/// 严格模式：某一层已声明但缺具体键时，在该层显式缺省，不向下回填。
pub fn resolve_strict(config: &Config, ward: &str, team: &str) -> OverlayResult {
    let team_prefix = format!("team.{team}.");
    let ward_prefix = format!("ward.{ward}.");
    if config.overrides.keys().any(|k| k.starts_with(&team_prefix)) {
        return match resolve_category(config, ward, team) {
            r if r.layer == OverlayLayer::Team => r,
            _ => OverlayResult {
                category: None,
                layer: OverlayLayer::Team,
                note: "班组层已配置但缺该项：显式缺省，不回填下一层".to_string(),
            },
        };
    }
    if config.overrides.keys().any(|k| k.starts_with(&ward_prefix)) {
        return match resolve_category(config, ward, team) {
            r if r.layer == OverlayLayer::Ward => r,
            _ => OverlayResult {
                category: None,
                layer: OverlayLayer::Ward,
                note: "科室层已配置但缺该项：显式缺省，不回填下一层".to_string(),
            },
        };
    }
    resolve_category(config, ward, team)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchSource {
    Exact,
    Alias,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchOutcome {
    pub category: String,
    pub source: MatchSource,
    pub evidence: String,
}

/// 精确键优先 → 别名表回退 → 归「待人工归类」。
pub fn match_key(config: &Config, key: &str) -> MatchOutcome {
    if let Some(v) = config.fault_vocabulary.get(key) {
        return MatchOutcome {
            category: key.to_string(),
            source: MatchSource::Exact,
            evidence: format!("精确键 {key}（{} 个术语）", v.len()),
        };
    }
    if let Some(v) = config.aliases.get(key) {
        return MatchOutcome {
            category: v.clone(),
            source: MatchSource::Alias,
            evidence: format!("别名键 {key}→{v}"),
        };
    }
    MatchOutcome {
        category: dfa_core::PENDING_MANUAL.to_string(),
        source: MatchSource::None,
        evidence: "精确键与别名表均未命中 → 待人工归类".to_string(),
    }
}
