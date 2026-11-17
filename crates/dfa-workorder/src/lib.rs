//! `dfa-workorder` —— 出站写客户端：写前必填与枚举校验、幂等键、
//! 逐条回执、失败不中断、指数退避与可重放语义。

pub mod client;
pub mod precheck;
pub mod receipt;

pub use client::*;
pub use precheck::*;
pub use receipt::*;
