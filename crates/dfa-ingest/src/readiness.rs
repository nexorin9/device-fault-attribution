//! 就绪三值（源 `InventoryHealth{Scanning, Ready, Unavailable}` 转义）：
//! 任一源不可用 → 聚合态不可用（Unknown），界面显示「工单源不可用」，绝不显示「无故障」。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Readiness {
    Scanning,
    Ready,
    Unavailable,
}

impl Readiness {
    pub fn label(&self) -> &'static str {
        match self {
            Readiness::Scanning => "就绪中",
            Readiness::Ready => "就绪",
            Readiness::Unavailable => "不可用",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHealth {
    pub workorders: Readiness,
    pub devices: Readiness,
    pub teams: Readiness,
}

impl Default for SourceHealth {
    fn default() -> Self {
        SourceHealth {
            workorders: Readiness::Scanning,
            devices: Readiness::Scanning,
            teams: Readiness::Scanning,
        }
    }
}

impl SourceHealth {
    pub fn aggregate(&self) -> Readiness {
        let all = [self.workorders, self.devices, self.teams];
        if all.iter().any(|s| *s == Readiness::Unavailable) {
            Readiness::Unavailable
        } else if all.iter().all(|s| *s == Readiness::Ready) {
            Readiness::Ready
        } else {
            Readiness::Scanning
        }
    }

    /// 源未就绪时，空列表 ≠ 无故障。
    pub fn banner(&self) -> String {
        match self.aggregate() {
            Readiness::Unavailable => "工单源不可用（请检查地址与令牌，稍后重试）".to_string(),
            Readiness::Scanning => "正在读取工单源…".to_string(),
            Readiness::Ready => "工单源就绪".to_string(),
        }
    }

    pub fn empty_state_text(&self, item_count: usize) -> String {
        match self.aggregate() {
            Readiness::Unavailable => "工单源不可用".to_string(),
            Readiness::Scanning => "正在读取工单源，暂未形成结论".to_string(),
            Readiness::Ready if item_count == 0 => "确实没有新工单".to_string(),
            Readiness::Ready => format!("当前 {item_count} 条待处理"),
        }
    }
}
