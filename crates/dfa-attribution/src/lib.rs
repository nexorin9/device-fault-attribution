//! `dfa-attribution` —— 描述归并三级链路、跨工单同源语义判定（三值）、
//! 根因假设与证据引用校验、核查步骤排序、规则守门与草稿边界、人工确认与降级编排。

pub mod confirm;
pub mod degrade;
pub mod merge;
pub mod rca;
pub mod same_source;
pub mod steps;

pub use confirm::*;
pub use degrade::*;
pub use merge::*;
pub use rca::*;
pub use same_source::*;
pub use steps::*;
