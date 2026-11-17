//! 设计 token：色板与语义色集中一处（本文件是唯一允许出现裸色值的源码），
//! 8px 间距栅格与字体层级统一。**源码其它位置不得出现裸色值**。

use dfa_core::ClusterState;

// ── 色板（裸色值只在本文件）────────────────────────────────────
pub const PRIMARY: &str = "#1F3A5F";
pub const SURFACE: &str = "#F7F9FC";
pub const BORDER: &str = "#D7DEE8";
pub const TEXT: &str = "#1B2430";
pub const MUTED: &str = "#6B7787";
pub const OK: &str = "#2E7D32";
pub const OBSERVING: &str = "#E07B00";
pub const CONFIRMED: &str = "#1D6FB8";
pub const ORDERED: &str = "#5B4BA8";
pub const CLOSED: &str = "#2E7D32";
pub const UNAVAILABLE: &str = "#9AA4B2";
pub const PENDING_VERIFY: &str = "#B26A00";
pub const ERROR: &str = "#C62828";

// RGB 元组供原生窗口层使用（避免在其它源码里写 #RRGGBB）。
pub const PRIMARY_RGB: (u8, u8, u8) = (31, 58, 95);
pub const SURFACE_RGB: (u8, u8, u8) = (247, 249, 252);
pub const TEXT_RGB: (u8, u8, u8) = (27, 36, 48);
pub const OK_RGB: (u8, u8, u8) = (46, 125, 50);
pub const OBSERVING_RGB: (u8, u8, u8) = (224, 123, 0);
pub const ERROR_RGB: (u8, u8, u8) = (198, 40, 40);

pub const GRID: usize = 8;

pub fn spacing(units: usize) -> usize {
    GRID * units
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontScale {
    pub title_px: u16,
    pub title_line: u16,
    pub body_px: u16,
    pub body_line: u16,
    pub caption_px: u16,
    pub caption_line: u16,
    pub numeric_px: u16,
    pub numeric_line: u16,
}

pub const FONT_SCALE: FontScale = FontScale {
    title_px: 18,
    title_line: 28,
    body_px: 14,
    body_line: 22,
    caption_px: 12,
    caption_line: 18,
    numeric_px: 13,
    numeric_line: 20,
};

/// 状态语义色。颜色只是辅助，必须同时给出说明文案（色觉障碍可读）。
pub fn semantic_color(state: ClusterState) -> &'static str {
    match state {
        ClusterState::Observing => OBSERVING,
        ClusterState::ConfirmedSameSource => CONFIRMED,
        ClusterState::Ordered => ORDERED,
        ClusterState::Closed => CLOSED,
    }
}

/// 不靠颜色单独承载信息：每个状态都配一句说明。
pub fn semantic_note(state: ClusterState) -> &'static str {
    match state {
        ClusterState::Observing => "观察中：未经人工确认，不会写出去",
        ClusterState::ConfirmedSameSource => "已确认同源：可批量开单",
        ClusterState::Ordered => "已开单：逐条回执可回查",
        ClusterState::Closed => "已闭环：不再进入待确认队列",
    }
}

pub fn unavailable_color() -> &'static str {
    UNAVAILABLE
}

pub fn pending_color() -> &'static str {
    PENDING_VERIFY
}

pub fn error_color() -> &'static str {
    ERROR
}

/// 禁用态：降饱和 + 说明文案。
pub fn disabled_style() -> (&'static str, &'static str) {
    (UNAVAILABLE, "禁用：当前条件不满足，请先补齐必填项")
}
