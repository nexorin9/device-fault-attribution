use dfa_cluster::{decide, fields_to_replenish, kernel, time_window_ok};
use dfa_core::{Device, PENDING_MANUAL};

fn device() -> Device {
    Device {
        device_id: "DEV-EXAMPLE-01".into(),
        model: "M-EXAMPLE".into(),
        serial: "SER-EXAMPLE-01".into(),
        firmware: "FW-EXAMPLE".into(),
        ward: "设备科".into(),
        purchase_batch: "LOT-EXAMPLE-01".into(),
    }
}

#[test]
fn missing_firmware_is_incomparable_with_field_names() {
    let mut d = device();
    d.firmware.clear();
    let r = decide(&kernel(0), &d, "不制冷", None);
    assert!(!r.comparable);
    assert!(r.missing.contains(&"firmware".to_string()));
    assert!(r.cluster_key.is_none());
}

#[test]
fn missing_batch_is_incomparable_with_field_names() {
    let mut d = device();
    d.purchase_batch.clear();
    let r = decide(&kernel(0), &d, "不制冷", None);
    assert!(!r.comparable);
    assert!(r.missing.contains(&"purchase_batch".to_string()));
}

#[test]
fn same_model_but_different_batch_forms_different_key() {
    let k = kernel(0);
    let mut d2 = device();
    d2.purchase_batch = "LOT-EXAMPLE-02".into();
    let a = decide(&k, &device(), "不制冷", None);
    let b = decide(&k, &d2, "不制冷", None);
    assert!(a.comparable && b.comparable);
    assert_ne!(a.cluster_key, b.cluster_key);
}

#[test]
fn out_of_vocabulary_becomes_pending_manual_without_new_category() {
    let r = decide(&kernel(0), &device(), "完全没见过的怪现象", None);
    assert!(r.out_of_vocabulary);
    assert_eq!(r.category, PENDING_MANUAL);
    assert!(!r.comparable);
}

#[test]
fn time_window_expired_gives_no_same_source_conclusion() {
    let k = kernel(60);
    assert!(time_window_ok(1000, 1030, &k));
    assert!(!time_window_ok(1000, 2000, &k));
}

#[test]
fn replenish_guidance_lists_missing_fields() {
    let guide = fields_to_replenish(&["firmware".into(), "purchase_batch".into()]);
    assert_eq!(guide.len(), 2);
    assert!(guide[0].contains("设备台账补录"));
}
