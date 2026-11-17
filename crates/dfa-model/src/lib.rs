//! `dfa-model` —— 单一 ModelGateway（OpenAI-compatible）：结构化输出 schema 校验、
//! 五类错误语义、指数退避重试、可审计调用元数据、模型不可达降级。

pub mod degrade;
pub mod errors;
pub mod gateway;
pub mod http;
pub mod meta;
pub mod schema;

pub use degrade::*;
pub use errors::*;
pub use gateway::*;
pub use http::*;
pub use meta::*;
pub use schema::*;
