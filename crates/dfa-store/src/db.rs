//! SQLite schema 与版本迁移。常驻进程独占写库，窗口只读快照文件。

use rusqlite::Connection;
use std::path::Path;
use thiserror::Error;

pub const SCHEMA_VERSION: i32 = 2;
pub const TABLES: [&str; 9] = [
    "devices",
    "work_orders",
    "clusters",
    "cluster_members",
    "confirmations",
    "receipts",
    "audit_chain",
    "model_calls",
    "snapshots",
];

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("SQLite 错误: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("快照版本过旧: {0}")]
    SnapshotVersion(String),
    #[error("审计链校验失败: {0}")]
    Audit(String),
}

/// 单写者 SQLite 句柄。窗口进程不持有此对象。
pub struct Store {
    pub conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let conn = Connection::open(path)?;
        let store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Store, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    /// v1 → v2 迁移：补齐 v2 新增的表与列，并把 schema 版本写回 meta 表。
    pub fn migrate(&self) -> Result<i32, StoreError> {
        let before = self.schema_version()?;
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS devices (
                device_id TEXT PRIMARY KEY,
                model TEXT NOT NULL DEFAULT '',
                serial TEXT NOT NULL DEFAULT '',
                firmware TEXT NOT NULL DEFAULT '',
                ward TEXT NOT NULL DEFAULT '',
                purchase_batch TEXT NOT NULL DEFAULT '',
                pending_verify INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS work_orders (
                workorder_no TEXT PRIMARY KEY,
                device_id TEXT NOT NULL DEFAULT '',
                fault_text TEXT NOT NULL DEFAULT '',
                fault_code TEXT NOT NULL DEFAULT '',
                reported_at TEXT NOT NULL DEFAULT '',
                reported_epoch INTEGER NOT NULL DEFAULT 0,
                reporter_dept TEXT NOT NULL DEFAULT '',
                pii_stripped INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS clusters (
                cluster_id TEXT PRIMARY KEY,
                cluster_key TEXT NOT NULL DEFAULT '',
                state TEXT NOT NULL DEFAULT 'Observing',
                fault_category TEXT NOT NULL DEFAULT '',
                draft INTEGER NOT NULL DEFAULT 1,
                first_seen_at TEXT NOT NULL DEFAULT '',
                earliest_epoch INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS cluster_members (
                cluster_id TEXT NOT NULL,
                workorder_no TEXT NOT NULL,
                device_id TEXT NOT NULL DEFAULT '',
                evidence_snapshot TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (cluster_id, workorder_no)
            );
            CREATE TABLE IF NOT EXISTS confirmations (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                cluster_id TEXT NOT NULL,
                actor TEXT NOT NULL,
                ts INTEGER NOT NULL,
                member_snapshot TEXT NOT NULL DEFAULT '',
                before_state TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS receipts (
                workorder_no TEXT NOT NULL,
                cluster_id TEXT NOT NULL,
                outcome TEXT NOT NULL,
                retryable INTEGER NOT NULL DEFAULT 0,
                http_status INTEGER NOT NULL DEFAULT 0,
                body_digest TEXT NOT NULL DEFAULT '',
                retry_count INTEGER NOT NULL DEFAULT 0,
                idempotency_key TEXT NOT NULL,
                PRIMARY KEY (cluster_id, workorder_no, idempotency_key)
            );
            CREATE TABLE IF NOT EXISTS audit_chain (
                seq INTEGER PRIMARY KEY,
                ts INTEGER NOT NULL,
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                prev_hash TEXT NOT NULL,
                entry_hash TEXT NOT NULL,
                cluster_id TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS model_calls (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                model TEXT NOT NULL,
                request_digest TEXT NOT NULL,
                response_digest TEXT NOT NULL,
                elapsed_ms INTEGER NOT NULL,
                retries INTEGER NOT NULL,
                degraded INTEGER NOT NULL,
                draft INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS snapshots (
                name TEXT PRIMARY KEY,
                payload TEXT NOT NULL,
                written_at INTEGER NOT NULL
            );
            ",
        )?;
        self.conn.execute(
            "INSERT INTO schema_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [SCHEMA_VERSION.to_string()],
        )?;
        Ok(before)
    }

    pub fn schema_version(&self) -> Result<i32, StoreError> {
        let exists: i64 = self.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='schema_meta'",
            [],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Ok(0);
        }
        let value: Result<String, _> = self.conn.query_row(
            "SELECT value FROM schema_meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        );
        Ok(value.unwrap_or_default().parse().unwrap_or(0))
    }

    pub fn table_names(&self) -> Result<Vec<String>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

/// 构造一个 v1 样例库（只含 3 张旧表，版本 = 1），供迁移测试使用。
pub fn create_v1_database(path: &Path) -> Result<(), StoreError> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        CREATE TABLE schema_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        INSERT INTO schema_meta(key, value) VALUES('schema_version', '1');
        CREATE TABLE devices (device_id TEXT PRIMARY KEY, model TEXT);
        CREATE TABLE work_orders (workorder_no TEXT PRIMARY KEY, device_id TEXT);
        CREATE TABLE clusters (cluster_id TEXT PRIMARY KEY, cluster_key TEXT);
        ",
    )?;
    conn.execute(
        "INSERT INTO devices(device_id, model) VALUES('DEV-EXAMPLE-01', '监护仪 M-220')",
        [],
    )?;
    Ok(())
}
