//! 四态基础组件：按钮、输入框、标签、表格行、状态徽标。
//! 每个组件都能渲染 hover / active / disabled / loading 四态；
//! 禁用态降饱和并附说明文案，不靠颜色单独承载信息。

use serde::{Deserialize, Serialize};

use crate::tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetState {
    Normal,
    Hover,
    Active,
    Disabled,
    Loading,
}

pub const FOUR_STATES: [WidgetState; 4] = [
    WidgetState::Hover,
    WidgetState::Active,
    WidgetState::Disabled,
    WidgetState::Loading,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetKind {
    Button,
    Input,
    Label,
    TableRow,
    StatusBadge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetRender {
    pub kind: WidgetKind,
    pub label: String,
    pub state: WidgetState,
    pub color_token: &'static str,
    pub note: String,
    pub interactive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Widget {
    pub kind: WidgetKind,
    pub label: String,
    pub semantic: &'static str,
}

impl Widget {
    pub fn button(label: &str) -> Self {
        Widget {
            kind: WidgetKind::Button,
            label: label.to_string(),
            semantic: tokens::PRIMARY,
        }
    }

    pub fn input(label: &str) -> Self {
        Widget {
            kind: WidgetKind::Input,
            label: label.to_string(),
            semantic: tokens::BORDER,
        }
    }

    pub fn label(label: &str) -> Self {
        Widget {
            kind: WidgetKind::Label,
            label: label.to_string(),
            semantic: tokens::TEXT,
        }
    }

    pub fn table_row(label: &str) -> Self {
        Widget {
            kind: WidgetKind::TableRow,
            label: label.to_string(),
            semantic: tokens::SURFACE,
        }
    }

    pub fn status_badge(label: &str, color: &'static str) -> Self {
        Widget {
            kind: WidgetKind::StatusBadge,
            label: label.to_string(),
            semantic: color,
        }
    }

    pub fn render(&self, state: WidgetState) -> WidgetRender {
        let (color_token, note, interactive) = match state {
            WidgetState::Normal => (
                self.semantic,
                "可操作".to_string(),
                true,
            ),
            WidgetState::Hover => (
                self.semantic,
                "悬停：即将可点".to_string(),
                true,
            ),
            WidgetState::Active => (
                self.semantic,
                "已按下：操作生效中".to_string(),
                true,
            ),
            WidgetState::Disabled => {
                let (c, n) = tokens::disabled_style();
                (c, n.to_string(), false)
            }
            WidgetState::Loading => (
                tokens::UNAVAILABLE,
                "处理中：已禁用重复点击".to_string(),
                false,
            ),
        };
        WidgetRender {
            kind: self.kind,
            label: self.label.clone(),
            state,
            color_token,
            note,
            interactive,
        }
    }

    /// 四态均可渲染（发布态自检）。
    pub fn render_four_states(&self) -> Vec<WidgetRender> {
        FOUR_STATES.iter().map(|s| self.render(*s)).collect()
    }
}

pub fn basic_widgets() -> Vec<Widget> {
    vec![
        Widget::button("确认并批量开单"),
        Widget::input("故障类别"),
        Widget::label("簇卡片"),
        Widget::table_row("设备号 | 型号 | 固件 | 批次"),
        Widget::status_badge("观察中", tokens::OBSERVING),
    ]
}
