//! 边界与异常输入处理：非法字节 / 不可见字符识别、超长字段截断留痕、脏行隔离。
//! 入口一律显式标「待核实 / 待人工归类」，不静默截断。

use serde::{Deserialize, Serialize};

use crate::types::WorkOrder;

pub const DEFAULT_MAX_FIELD_LEN: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanResult {
    pub text: String,
    pub flags: Vec<String>,
    pub truncated: bool,
    pub original_len: usize,
    pub truncation_pos: usize,
}

/// 非法字节序列以 U+FFFD / 控制字符形式进入；识别后标「待核实」，不静默丢弃。
pub fn clean_text(raw: &str, max_len: usize) -> CleanResult {
    let mut flags = Vec::new();
    let mut text = String::new();
    for ch in raw.chars() {
        if ch == '\u{FFFD}' {
            flags.push("PENDING_VERIFY:ILLEGAL_BYTE".to_string());
            continue;
        }
        if ch.is_control() && ch != '\n' && ch != '\t' {
            flags.push("PENDING_VERIFY:INVISIBLE_CHAR".to_string());
            continue;
        }
        text.push(ch);
    }
    if text.trim().is_empty() && !raw.is_empty() {
        flags.push("PENDING_VERIFY:EMPTY_OR_BLANK".to_string());
    }
    let original_len = text.chars().count();
    let mut truncated = false;
    let mut truncation_pos = 0;
    let cap = max_len.max(1);
    if original_len > cap {
        truncation_pos = cap;
        text = text.chars().take(cap).collect();
        flags.push(format!("TRUNCATED:{original_len}->{cap}"));
        truncated = true;
    }
    CleanResult {
        text,
        flags,
        truncated,
        original_len,
        truncation_pos,
    }
}

/// JSONL 逐行解析：单行失败不影响其余行，失败行计数进运行摘要。
pub fn parse_workorder_lines(lines: &[String]) -> (Vec<WorkOrder>, usize) {
    let mut ok = Vec::new();
    let mut failed = 0usize;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<WorkOrder>(trimmed) {
            Ok(wo) if !wo.workorder_no.trim().is_empty() && !wo.device_id.trim().is_empty() => {
                let cleaned = clean_text(&wo.fault_text, DEFAULT_MAX_FIELD_LEN);
                let mut wo = wo;
                wo.fault_text = cleaned.text;
                ok.push(wo);
            }
            _ => failed += 1,
        }
    }
    (ok, failed)
}

/// 清洗结果是否要求走「待核实」分流（而不是参与同源判定）。
pub fn needs_verification(result: &CleanResult) -> bool {
    result
        .flags
        .iter()
        .any(|f| f.starts_with("PENDING_VERIFY"))
}
