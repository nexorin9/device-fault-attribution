//! 受控词表以稳定标识符作 TOML 键：改名是迁移事件，必须被配置校验检出，
//! 绝不静默沿用旧名。

use serde::{Deserialize, Serialize};

/// 旧标识符 → 新标识符。新增迁移项只需在此登记。
pub const KNOWN_RENAMES: &[(&str, &str)] = &[
    ("REFUSAL_TO_COOL_V0", "REFUSAL_TO_COOL"),
    ("NO_RESPONSE_OLD", "NO_RESPONSE"),
    ("ALARM_ONLY_LEGACY", "ALARM_ONLY"),
    ("INFUSION_BLOCKED_V1", "INFUSION_BLOCKED"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationIssue {
    pub old: String,
    pub new: String,
    pub message: String,
}

/// 扫描 `[fault_vocabulary]` 段内的键；出现旧标识符即报迁移错误并指出新旧名。
pub fn detect_renames(text: &str) -> Vec<MigrationIssue> {
    let mut issues = Vec::new();
    let mut in_vocabulary = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_vocabulary = line == "[fault_vocabulary]";
            continue;
        }
        if !in_vocabulary || line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim().trim_matches('"');
            for (old, new) in KNOWN_RENAMES {
                if key == *old {
                    issues.push(MigrationIssue {
                        old: (*old).to_string(),
                        new: (*new).to_string(),
                        message: format!(
                            "词表标识符 {old} 已改名为 {new}；请先迁移 config.toml，不静默沿用旧名"
                        ),
                    });
                }
            }
        }
    }
    issues
}

/// doctor 命令用的迁移检查入口。
pub fn doctor_check(text: &str) -> Result<String, String> {
    let issues = detect_renames(text);
    if issues.is_empty() {
        Ok("词表标识符：无待迁移改名".to_string())
    } else {
        Err(issues
            .iter()
            .map(|i| i.message.clone())
            .collect::<Vec<_>>()
            .join("；"))
    }
}
