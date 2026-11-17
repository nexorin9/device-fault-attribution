//! `dfa-registry` —— 权威设备注册表：解析成功才纳入、序列号 + 型号双因子绑定、
//! 身份歧义拒绝、待核实队列与只读不回写约束。

pub mod binding;
pub mod registry;

pub use binding::*;
pub use registry::*;
