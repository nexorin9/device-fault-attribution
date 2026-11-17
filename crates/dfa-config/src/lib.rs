//! `dfa-config` —— 只读 TOML 声明式配置、硬约束求值、词表改名迁移检测、
//! 三层叠加与精确键优先 / 别名回退匹配器。配置只读，永不回写。

pub mod constraints;
pub mod loader;
pub mod migration;
pub mod overlay;

pub use constraints::*;
pub use loader::*;
pub use migration::*;
pub use overlay::*;
