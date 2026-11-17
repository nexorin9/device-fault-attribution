//! 快照原子读写：写临时文件 + 原子替换，窗口永远读到完整的一份。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::db::{StoreError, SCHEMA_VERSION};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEnvelope {
    pub schema_version: i32,
    pub name: String,
    pub payload: String,
    pub written_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRead {
    pub envelope: SnapshotEnvelope,
    /// 旧版本快照仍可读，但给出降级提示，不静默当成当前版本。
    pub degraded_note: Option<String>,
}

pub fn write_snapshot(path: &Path, name: &str, payload: &str, ts: i64) -> Result<PathBuf, StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let envelope = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        name: name.to_string(),
        payload: payload.to_string(),
        written_at: ts,
    };
    let bytes = serde_json::to_string(&envelope)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(path.to_path_buf())
}

pub fn read_snapshot(path: &Path) -> Result<SnapshotRead, StoreError> {
    let raw = fs::read_to_string(path)?;
    let envelope: SnapshotEnvelope = serde_json::from_str(&raw)?;
    let degraded_note = if envelope.schema_version < SCHEMA_VERSION {
        Some(format!(
            "快照版本 v{} 低于当前 v{}，已按兼容方式读取并建议重建",
            envelope.schema_version, SCHEMA_VERSION
        ))
    } else {
        None
    };
    Ok(SnapshotRead {
        envelope,
        degraded_note,
    })
}

/// 模拟原子替换中断：只留下 .tmp，旧快照必须仍完整可读。
pub fn simulate_interrupted_write(path: &Path, partial: &str) -> Result<(), StoreError> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, partial)?;
    Ok(())
}
