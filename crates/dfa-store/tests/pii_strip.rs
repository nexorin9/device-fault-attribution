use dfa_core::WorkOrder;
use dfa_store::{strip_pii, Store};

#[test]
fn strip_removes_four_identifier_kinds() {
    let s = strip_pii(
        "患者姓名：张三 住院号 12345678 手机 13800138000 身份证 110101199001011234 不制冷",
    );
    assert!(!s.text.contains("13800138000"));
    assert!(!s.text.contains("110101199001011234"));
    assert!(!s.text.contains("张三"));
    assert!(!s.text.contains("12345678"));
    assert!(s.removed.len() >= 4);
    assert!(s.audit_note.starts_with("PII_STRIP"));
}

#[test]
fn full_table_scan_has_no_patient_identifiers() {
    let mut store = Store::in_memory().unwrap();
    let wo = WorkOrder {
        workorder_no: "WO-EXAMPLE-001".into(),
        device_id: "DEV-EXAMPLE-01".into(),
        fault_text: "患者姓名：张三 住院号 12345678 手机 13800138000 不制冷".into(),
        fault_code: "REFUSAL_TO_COOL".into(),
        ..WorkOrder::default()
    };
    let stripped = store.insert_work_order(&wo, 1741000000).unwrap();
    assert!(!stripped.removed.is_empty());
    let dump = store.dump_all_text().unwrap();
    assert!(!dump.contains("13800138000"));
    assert!(!dump.contains("张三"));
    assert!(dump.contains("[REDACTED]"));
    assert!(store.verify_audit_chain().unwrap());
}
