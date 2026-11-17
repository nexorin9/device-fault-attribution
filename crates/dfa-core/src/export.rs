//! 簇清单导出（CSV / Markdown）。只导出已确认同源及之后的簇；
//! 样例标识一律占位，导出前扫描断言无「真实样式」批次号 / 序列号 / 患者标识。

use std::fmt;

use crate::state::ClusterState;
use crate::types::Cluster;

pub const CSV_HEADER: &str = "cluster_key,device_id,model,firmware,purchase_batch,ward,fault_category,first_seen_at,member_count";
pub const DRAFT_LABEL: &str = "草稿 · 待人工确认";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportError(pub String);

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "导出校验失败：{}", self.0)
    }
}

fn exportable(c: &Cluster) -> bool {
    matches!(
        c.state,
        ClusterState::ConfirmedSameSource | ClusterState::Ordered | ClusterState::Closed
    )
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn export_csv(clusters: &[Cluster]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for c in clusters.iter().filter(|c| exportable(c)) {
        for m in &c.members {
            let row = [
                c.cluster_key.as_str(),
                m.device_id.as_str(),
                m.model.as_str(),
                m.firmware.as_str(),
                m.purchase_batch.as_str(),
                m.ward.as_str(),
                c.fault_category.as_str(),
                c.first_seen_at.as_str(),
                &c.member_count().to_string(),
            ];
            out.push_str(
                &row.iter()
                    .map(|v| csv_escape(v))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            out.push('\n');
        }
    }
    out
}

pub fn export_markdown(clusters: &[Cluster]) -> String {
    let mut out = String::from("# 同批设备故障簇清单\n\n");
    let kept: Vec<&Cluster> = clusters.iter().filter(|c| exportable(c)).collect();
    if kept.is_empty() {
        out.push_str("（当前没有已确认同源的簇，观察中与待核实的簇不进入对外清单。）\n");
        return out;
    }
    for c in &kept {
        out.push_str(&format!(
            "## 簇 {} · {} · {} 台设备\n\n",
            c.cluster_id,
            c.state.label(),
            c.member_count()
        ));
        out.push_str("| 设备号 | 型号 | 固件 | 到货批次 | 科室 | 故障类别 |\n|---|---|---|---|---|---|\n");
        for m in &c.members {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                m.device_id, m.model, m.firmware, m.purchase_batch, m.ward, c.fault_category
            ));
        }
        out.push_str("\n**证据字段**：\n\n");
        for e in &c.evidence {
            out.push_str(&format!("- {e}\n"));
        }
        out.push_str(&format!("\n**根因假设（{DRAFT_LABEL}）**：\n\n"));
        for h in &c.hypothesis {
            out.push_str(&format!("- {h}\n"));
        }
        out.push_str("\n**核查步骤（按可证伪优先序）**：\n\n");
        for (i, s) in c.steps.iter().enumerate() {
            out.push_str(&format!("{}. {s}\n", i + 1));
        }
        out.push('\n');
    }
    out
}

/// 是否形如真实批次号：B2023Q4-07 / LOT20240115-003 等。
fn looks_like_real_lot(token: &str) -> bool {
    let b = token.as_bytes();
    let has_year = b.len() >= 5
        && (b[0] == b'B' || token.starts_with("LOT"))
        && b[1..5].iter().all(|c| c.is_ascii_digit());
    let has_quarter = token.contains('Q') && token.contains('-');
    has_year && has_quarter
}

fn longest_digit_run(s: &str) -> usize {
    let mut cur = 0;
    let mut best = 0;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

/// 导出前扫描：命中真实样式标识即拒绝导出（不允许可被误认为真实的样例混进材料）。
pub fn assert_no_real_identifiers(text: &str) -> Result<(), ExportError> {
    for token in text.split(|c: char| c.is_whitespace() || c == ',' || c == '|' || c == '"') {
        let clean = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '<' && c != '>');
        if looks_like_real_lot(clean) {
            return Err(ExportError(format!("发现真实样式批次号 {clean}")));
        }
        if longest_digit_run(clean) >= 11 {
            return Err(ExportError(format!(
                "发现疑似序列号 / 患者标识的长数字串 {clean}"
            )));
        }
        if clean.starts_with("SN") && clean.len() > 6 {
            return Err(ExportError(format!("发现真实样式序列号 {clean}")));
        }
    }
    Ok(())
}

/// 把误入的真实样式标识替换为占位标识，用于导出前的兜底脱敏。
pub fn mask_identifiers(text: &str) -> String {
    let mut out = String::new();
    for token in text.split_inclusive(|c: char| c.is_whitespace() || c == ',' || c == '|') {
        let bare = token.trim_end_matches(|c: char| c.is_whitespace() || c == ',' || c == '|');
        if looks_like_real_lot(bare) || (bare.starts_with("SN") && bare.len() > 6) {
            let tail: String = token
                .chars()
                .rev()
                .take_while(|c| c.is_whitespace() || *c == ',' || *c == '|')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            out.push_str("<EXAMPLE_LOT>");
            out.push_str(&tail);
        } else {
            out.push_str(token);
        }
    }
    out
}
