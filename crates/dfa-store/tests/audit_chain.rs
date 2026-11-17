use dfa_store::{AuditLog, Store, GENESIS_HASH, SCHEMA_VERSION, TABLES};

#[test]
fn three_appends_have_continuous_seq_and_valid_hash_chain() {
    let mut log = AuditLog::new();
    for i in 0..3 {
        log.append(
            "dfa-core",
            "CLUSTER_FORMED",
            &format!("WO-{i}"),
            "payload",
            "C-001",
            100 + i,
        );
    }
    let e = log.entries();
    assert_eq!(e.len(), 3);
    assert_eq!(e[0].prev_hash, GENESIS_HASH);
    assert_eq!(e[2].prev_hash, e[1].entry_hash);
    for (i, x) in e.iter().enumerate() {
        assert_eq!(x.seq, i as u64 + 1);
    }
    assert!(log.verify_chain());
    assert_eq!(log.for_cluster("C-001").len(), 3);
    assert_eq!(log.for_target("WO-1").len(), 1);
}

#[test]
fn sqlite_append_and_chain_verification() {
    let mut store = Store::in_memory().unwrap();
    for i in 0..3 {
        store
            .append_audit("dfa-store", "RECEIPT", &format!("WO-{i}"), "body", "C-001", 200 + i)
            .unwrap();
    }
    assert_eq!(store.audit_entries().unwrap().len(), 3);
    assert!(store.verify_audit_chain().unwrap());
    assert_eq!(store.audit_for_cluster("C-001").unwrap().len(), 3);
    assert_eq!(store.audit_for_target("WO-2").unwrap().len(), 1);
}

#[test]
fn tampering_is_detected() {
    let mut store = Store::in_memory().unwrap();
    store.append_audit("a", "ACTION", "t", "p", "C-1", 1).unwrap();
    store.append_audit("a", "ACTION", "t", "p", "C-1", 2).unwrap();
    // 直接用 SQL 模拟篡改；接口层本身不提供 update / delete。
    store
        .conn
        .execute(
            "UPDATE audit_chain SET payload_digest = 'TAMPERED' WHERE seq = 1",
            rusqlite::params![],
        )
        .unwrap();
    assert!(!store.verify_audit_chain().unwrap());
}

#[test]
fn all_nine_tables_exist_and_version_is_current() {
    let store = Store::in_memory().unwrap();
    let names = store.table_names().unwrap();
    for t in TABLES {
        assert!(names.contains(&t.to_string()), "缺表 {t}");
    }
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, 2);
}
