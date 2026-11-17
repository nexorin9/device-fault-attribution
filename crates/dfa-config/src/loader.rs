//! 只读声明式 TOML 配置。程序永不回写本文件；凭据与策略分离。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;

use crate::migration::{detect_renames, MigrationIssue};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("配置读取失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML 解析失败: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("词表改名迁移错误: {0}")]
    Migration(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HardConstraintsSection {
    pub require_model: bool,
    pub require_firmware: bool,
    pub require_batch: bool,
    pub time_window_secs: i64,
}

impl Default for HardConstraintsSection {
    fn default() -> Self {
        HardConstraintsSection {
            require_model: true,
            require_firmware: true,
            require_batch: true,
            time_window_secs: 30 * 24 * 3600,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DispatchSection {
    pub priority_floor_for_critical_ward: String,
    pub default_priority: String,
    pub allowed_priorities: Vec<String>,
}

impl Default for DispatchSection {
    fn default() -> Self {
        DispatchSection {
            priority_floor_for_critical_ward: "P2".to_string(),
            default_priority: "P3".to_string(),
            allowed_priorities: vec![
                "P1".to_string(),
                "P2".to_string(),
                "P3".to_string(),
                "P4".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfirmBudgetSection {
    pub new_cluster: u32,
    pub recurring_cluster: u32,
    pub carry_strategy: String,
}

impl Default for ConfirmBudgetSection {
    fn default() -> Self {
        ConfirmBudgetSection {
            new_cluster: 4,
            recurring_cluster: 1,
            carry_strategy: "max".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub hard_constraints: HardConstraintsSection,
    pub dispatch: DispatchSection,
    pub fault_vocabulary: BTreeMap<String, Vec<String>>,
    pub aliases: BTreeMap<String, String>,
    pub team_capabilities: BTreeMap<String, Vec<String>>,
    pub confirm_budget: ConfirmBudgetSection,
    pub overrides: BTreeMap<String, String>,
}

impl Config {
    /// 只读加载：读文本 → TOML 解析 → 词表改名迁移校验。全程不写文件。
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = std::fs::read_to_string(path)?;
        Config::from_text(&text)
    }

    pub fn from_text(text: &str) -> Result<Config, ConfigError> {
        let issues: Vec<MigrationIssue> = detect_renames(text);
        if !issues.is_empty() {
            let msg = issues
                .iter()
                .map(|i| i.message.clone())
                .collect::<Vec<_>>()
                .join("；");
            return Err(ConfigError::Migration(msg));
        }
        let cfg: Config = toml::from_str(text)?;
        Ok(cfg)
    }

    /// 空段骨架时提供出厂默认策略（不是「给判定字段回填默认值」）。
    pub fn with_defaults(mut self) -> Config {
        if self.dispatch.allowed_priorities.is_empty() {
            self.dispatch = DispatchSection::default();
        }
        if self.confirm_budget.new_cluster == 0 {
            self.confirm_budget = ConfirmBudgetSection::default();
        }
        self
    }

    pub fn hard_constraints(&self) -> dfa_core::HardConstraints {
        dfa_core::HardConstraints {
            require_model: self.hard_constraints.require_model,
            require_firmware: self.hard_constraints.require_firmware,
            require_batch: self.hard_constraints.require_batch,
            time_window_secs: self.hard_constraints.time_window_secs,
        }
    }
}
