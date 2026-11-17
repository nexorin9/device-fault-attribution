//! 患者可识别标识剥离：工单自由文本落库前处理，剥离动作本身写审计。
//! 只处理设备级与工单级字段，不触碰患者数据。

use dfa_core::WorkOrder;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::{Store, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiiStrip {
    pub text: String,
    pub removed: Vec<String>,
    pub audit_note: String,
}

pub fn strip_pii(raw: &str) -> PiiStrip {
    let patterns: [(&str, &str); 4] = [
        (r"\b1[3-9][0-9]{9}\b", "手机号"),
        (r"\b[0-9]{17}[0-9Xx]\b", "身份证号"),
        (r"住院号[:：]?\s*[0-9]{4,}", "住院号"),
        (r"姓名[:：]\s*[\x{4e00}-\x{9fff}]{2,4}", "姓名"),
    ];
    let mut text = raw.to_string();
    let mut removed = Vec::new();
    for (pattern, label) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            for m in re.find_iter(&text.clone()) {
                removed.push(format!("{label}:{}", m.as_str()));
            }
            text = re.replace_all(&text, "[REDACTED]").to_string();
        }
    }
    let audit_note = if removed.is_empty() {
        "PII_SCAN:0".to_string()
    } else {
        format!("PII_STRIP:{} 项", removed.len())
    };
    PiiStrip {
        text,
        removed,
        audit_note,
    }
}

impl Store {
    /// 工单落库：先剥离患者可识别片段，再写入；剥离动作写一条审计。
    pub fn insert_work_order(&mut self, wo: &WorkOrder, ts: i64) -> Result<PiiStrip, StoreError> {
        let stripped = strip_pii(&wo.fault_text);
        self.conn.execute(
            "INSERT INTO work_orders(workorder_no, device_id, fault_text, fault_code, reported_at,
                                     reported_epoch, reporter_dept, pii_stripped)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(workorder_no) DO UPDATE SET
               fault_text = excluded.fault_text,
               pii_stripped = excluded.pii_stripped",
            params![
                wo.workorder_no,
                wo.device_id,
                stripped.text,
                wo.fault_code,
                wo.reported_at,
                wo.reported_epoch,
                wo.reporter_dept,
                !stripped.removed.is_empty(),
            ],
        )?;
        let _ = self.append_audit(
            "dfa-store",
            "PII_STRIP",
            &wo.workorder_no,
            &stripped.audit_note,
            "",
            ts,
        )?;
        Ok(stripped)
    }
}
