//! `dfa-cluster` —— 逐 tick 差分三态、确认预算只增不减、同源成簇硬约束门、
//! 簇形成 / 合并 / 拆分与演进留痕、幂等键与重放、基线判定器、窄主干编排与规模度量。

pub mod baseline;
pub mod budget;
pub mod cluster_key;
pub mod diff;
pub mod engine;
pub mod evolution;
pub mod idempotency;

pub use baseline::*;
pub use budget::*;
pub use cluster_key::*;
pub use diff::*;
pub use engine::*;
pub use evolution::*;
pub use idempotency::*;
