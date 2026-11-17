//! `dfa-desktop` —— 设计 token 与四态基础组件、单窗口两栏快照投影、
//! 加载 / 空 / 错误三态与二次确认、响应式两档、系统托盘计数与降级路径。
//! 默认构建只含纯逻辑层（自动化验证环境无显示服务）；
//! `--features native` 启用 egui/eframe 原生窗口。

pub mod states;
pub mod tokens;
pub mod tray;
pub mod widgets;
pub mod window;

#[cfg(feature = "native")]
pub mod native;

pub use states::*;
pub use tokens::*;
pub use tray::*;
pub use widgets::*;
pub use window::*;
