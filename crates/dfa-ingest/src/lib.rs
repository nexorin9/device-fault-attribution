//! `dfa-ingest` —— 三路入站轮询（台账分页 / 游标、工单源、班组能力每次重取）、
//! 就绪三值传播、事件扇入去重与顺序稳定、常驻生命周期与休眠唤醒 I/O 门控。

pub mod lifecycle;
pub mod queue;
pub mod readiness;
pub mod sources;

pub use lifecycle::*;
pub use queue::*;
pub use readiness::*;
pub use sources::*;
