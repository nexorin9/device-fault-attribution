//! 系统托盘（常驻进程侧）：待处理计数 + 菜单 + 点击唤起窗口。
//! 源产品没有 OS 级 toast / 气泡通道，故不做气泡；
//! 托盘不可用时降级为「窗口置顶徽标 + 应用内横幅」。

use serde::{Deserialize, Serialize};

use crate::window::SnapshotView;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayState {
    pub pending_count: usize,
    pub new_cluster_count: usize,
    pub available: bool,
    pub menu: Vec<String>,
}

impl Default for TrayState {
    fn default() -> Self {
        TrayState {
            pending_count: 0,
            new_cluster_count: 0,
            available: true,
            menu: vec![
                "打开窗口".to_string(),
                "导出簇清单".to_string(),
                "退出".to_string(),
            ],
        }
    }
}

impl TrayState {
    /// 托盘计数与窗口待处理数必须一致（同一份快照投影）。
    pub fn from_view(view: &SnapshotView) -> Self {
        TrayState {
            pending_count: view.pending_count(),
            new_cluster_count: view.cluster_cards.len(),
            available: true,
            menu: TrayState::default().menu,
        }
    }

    pub fn matches_window(&self, view: &SnapshotView) -> bool {
        self.pending_count == view.pending_count()
    }

    pub fn title(&self) -> String {
        if self.pending_count == 0 {
            "设备归因 · 无待处理".to_string()
        } else {
            format!("设备归因 · {} 条待处理", self.pending_count)
        }
    }

    pub fn menu_lines(&self) -> Vec<String> {
        let mut out = vec![format!("待处理条数：{}", self.pending_count)];
        out.push(format!("新增簇数：{}", self.new_cluster_count));
        out.extend(self.menu.iter().cloned());
        out
    }

    /// 托盘不可用 → 降级路径（窗口置顶徽标 + 应用内横幅），主路径不受影响。
    pub fn degraded_banner(&self) -> Option<String> {
        if self.available {
            None
        } else {
            Some(format!(
                "托盘不可用：改用窗口置顶徽标 + 应用内横幅（{} 条待处理）",
                self.pending_count
            ))
        }
    }

    pub fn click_action(&self) -> &'static str {
        "点击托盘唤起窗口并置前（不使用 OS toast）"
    }
}
