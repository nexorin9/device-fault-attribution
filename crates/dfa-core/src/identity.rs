//! 设备稳定身份与幂等键。
//! 身份只由「型号 + 序列号 + 到货批次」构成：固件升级、科室改名都不改变身份键；
//! 序列号或型号缺失即歧义拒绝，绝不按名称或科室猜身份。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 幂等键：sha256(cluster_id|workorder_no)，与时间戳、重试次数、字段顺序无关。
pub fn idempotency_key(cluster_id: &str, workorder_no: &str) -> String {
    sha256_hex(&format!("{cluster_id}|{workorder_no}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceStableId {
    pub key: String,
    pub model: String,
    pub serial: String,
    pub purchase_batch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentityResolution {
    Resolved(DeviceStableId),
    Ambiguous(String),
    Conflict(String),
}

impl IdentityResolution {
    pub fn is_resolved(&self) -> bool {
        matches!(self, IdentityResolution::Resolved(_))
    }

    pub fn label(&self) -> &'static str {
        match self {
            IdentityResolution::Resolved(_) => "可解析",
            IdentityResolution::Ambiguous(_) => "不可解析",
            IdentityResolution::Conflict(_) => "冲突",
        }
    }
}

impl DeviceStableId {
    /// 唯一的身份入口：只允许按双因子解析，不提供按 ward / 名称猜测的分支。
    pub fn from_parts(model: &str, serial: &str, batch: &str) -> IdentityResolution {
        if serial.trim().is_empty() {
            return IdentityResolution::Ambiguous("MISSING_SERIAL".to_string());
        }
        if model.trim().is_empty() {
            return IdentityResolution::Ambiguous("MISSING_MODEL".to_string());
        }
        let key = sha256_hex(&format!(
            "{}|{}|{}",
            model.trim(),
            serial.trim(),
            batch.trim()
        ));
        IdentityResolution::Resolved(DeviceStableId {
            key: key[..24].to_string(),
            model: model.trim().to_string(),
            serial: serial.trim().to_string(),
            purchase_batch: batch.trim().to_string(),
        })
    }
}
