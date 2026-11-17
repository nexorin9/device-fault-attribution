//! `dfa-store` —— SQLite schema / 迁移、只增不改审计链、快照原子读写、
//! 按簇反查与患者可识别标识剥离。常驻进程独占写库，窗口只读快照文件。

pub mod audit;
pub mod db;
pub mod pii;
pub mod query;
pub mod snapshot;

pub use audit::*;
pub use db::*;
pub use pii::*;
pub use query::*;
pub use snapshot::*;
