//! UI 状态与打磨：加载 / 空 / 错误三态、操作反馈、破坏性操作二次确认、
//! 响应式两档（1280 / 768）与表格打磨细节。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutMode {
    TwoColumn1280,
    Stacked768,
}

pub fn layout_mode(width: u32) -> LayoutMode {
    if width < 1024 {
        LayoutMode::Stacked768
    } else {
        LayoutMode::TwoColumn1280
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewState {
    Loading,
    Empty { count: usize },
    Error { code: String, text: String },
}

/// 就绪三值直接决定文案：源不可用时**绝不**显示「无故障」。
pub fn state_text(state: &ViewState, health: &str) -> String {
    match state {
        ViewState::Loading => "正在读取工单源…".to_string(),
        ViewState::Empty { count } => {
            if health == "不可用" {
                "工单源不可用（重试入口在状态栏）".to_string()
            } else if health == "就绪中" {
                "正在读取工单源，暂未形成结论".to_string()
            } else if *count == 0 {
                "确实没有新工单".to_string()
            } else {
                format!("当前 {count} 条待处理")
            }
        }
        ViewState::Error { code, text } => format!("{code}：{text}（可重试）"),
    }
}

pub fn never_says_no_fault(text: &str) -> bool {
    !text.contains("无故障")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionFeedback {
    pub banner: String,
    pub inline: String,
    pub loading: bool,
    pub disabled_repeat_click: bool,
}

/// 提交后按钮进入 loading 并禁用重复点击；成功 / 失败以行内反馈 + 顶部横幅呈现。
pub fn submit_feedback(success: bool, ok_count: usize, fail_count: usize) -> ActionFeedback {
    ActionFeedback {
        banner: if success {
            format!("批量开单完成：成功 {ok_count} 张，失败 {fail_count} 张（失败项可重放）")
        } else {
            "提交被拦下：请先补齐必填项".to_string()
        },
        inline: if success {
            format!("逐条回执：成功 {ok_count} / 可重放 {fail_count}")
        } else {
            "出站请求计数 = 0".to_string()
        },
        loading: false,
        disabled_repeat_click: true,
    }
}

pub fn submitting_feedback() -> ActionFeedback {
    ActionFeedback {
        banner: "正在逐条提交…".to_string(),
        inline: "按钮已禁用，避免重复点击".to_string(),
        loading: true,
        disabled_repeat_click: true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestructiveConfirm {
    pub open: bool,
    pub order_count: usize,
    pub models: Vec<String>,
    pub second_confirm: bool,
}

/// 批量开单属破坏性操作：确认框复述将要创建的单数与受影响设备型号。
pub fn confirm_text(dialog: &DestructiveConfirm) -> String {
    format!(
        "确认批量创建 {} 张工单？受影响型号：{}。取消即不产生任何出站请求。",
        dialog.order_count,
        if dialog.models.is_empty() {
            "（未选择成员）".to_string()
        } else {
            dialog.models.join(" / ")
        }
    )
}

pub fn cancel_outbound() -> usize {
    0
}

/// 长故障描述折叠展开：超过上限时截断并保留可展开标记。
pub fn polish_text(text: &str, max_len: usize) -> String {
    if text.is_empty() || text.trim().is_empty() {
        return "—".to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return text.to_string();
    }
    let head: String = chars[..max_len].iter().collect();
    format!("{head}… [展开全文，共 {} 字]", chars.len())
}

/// 空值统一显示「—」，不显示空白。
pub fn display_value(value: &str) -> String {
    if value.trim().is_empty() {
        "—".to_string()
    } else {
        value.to_string()
    }
}

/// 表格数字右对齐、证据字段等宽对齐（用空格补齐，便于等宽字体渲染）。
pub fn align_numeric(value: &str, width: usize) -> String {
    if value.len() >= width {
        value.to_string()
    } else {
        format!("{}{}", " ".repeat(width - value.len()), value)
    }
}
