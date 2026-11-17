//! `dfa-core` —— 医疗设备故障归因系统的公共领域地基。
//!
//! 领域类型、三值在场性、簇状态机、受控词表、稳定身份、确定性判定内核、
//! 簇清单导出、结构化日志与边界清洗。所有其它 crate 只依赖本层，不反向依赖。

pub mod cleaning;
pub mod decision;
pub mod export;
pub mod identity;
pub mod logging;
pub mod sighting;
pub mod state;
pub mod types;
pub mod vocabulary;

pub use cleaning::*;
pub use decision::*;
pub use export::*;
pub use identity::*;
pub use logging::*;
pub use sighting::*;
pub use state::*;
pub use types::*;
pub use vocabulary::*;
