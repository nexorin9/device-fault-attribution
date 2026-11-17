//! `dfa-cli` —— 运维排障通道：doctor 自检、snapshot 只读快照、
//! replay 离线重放、export 簇清单、confirm 人工确认批量开单，
//! 以及评测 / 规模度量 / 窗口投影的内部命令。每次调用有 5 秒硬上限语义。

pub mod commands;

pub use commands::*;

/// 命令入口（供 main.rs 与集成测试共用）。
pub fn run(args: Vec<String>) -> i32 {
    commands::run(args)
}
