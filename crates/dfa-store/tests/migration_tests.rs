use dfa_store::{create_v1_database, Store, SCHEMA_VERSION};
use tempfile::tempdir;

#[test]
fn v1_database_migrates_to_v2_without_losing_rows() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("v1.sqlite");
    create_v1_database(&path).unwrap();
    {
        let old = Store {
            conn: rusqlite::Connection::open(&path).unwrap(),
        };
        assert_eq!(old.schema_version().unwrap(), 1);
        assert_eq!(old.table_names().unwrap().len(), 4);
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    let names = store.table_names().unwrap();
    for t in [
        "cluster_members",
        "confirmations",
        "receipts",
        "audit_chain",
        "model_calls",
        "snapshots",
    ] {
        assert!(names.contains(&t.to_string()), "迁移后缺表 {t}");
    }
    let n: i64 = store
        .conn
        .query_row("SELECT count(*) FROM devices", rusqlite::params![], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn cluster_detail_query_returns_members_confirmations_receipts_and_audit() {
    let mut store = Store::in_memory().unwrap();
    store
        .upsert_cluster("C-001", "M|FW|LOT|CAT", "已确认同源", "REFUSAL_TO_COOL", "2025-03-03", 1741000000)
        .unwrap();
    store
        .upsert_member("C-001", "WO-001", "DEV-EXAMPLE-01", "model=M")
        .unwrap();
    store
        .insert_confirmation("C-001", "设备科值班工程师", 1741000001, "DEV-EXAMPLE-01", "观察中")
        .unwrap();
    store
        .append_audit("dfa-cluster", "Formed", "C-001", "evidence", "C-001", 1741000002)
        .unwrap();
    let detail = store.cluster_detail("C-001").unwrap();
    assert_eq!(detail.cluster_key, "M|FW|LOT|CAT");
    assert_eq!(detail.members.len(), 1);
    assert_eq!(detail.confirmations.len(), 1);
    assert_eq!(detail.audit_entries.len(), 1);
    assert!(detail.confirmations[0].contains("设备科值班工程师"));
}
