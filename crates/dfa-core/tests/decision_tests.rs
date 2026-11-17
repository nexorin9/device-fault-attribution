use dfa_core::{
    eval_hard_constraints, DeterministicKernel, Device, FaultVocabulary, HardConstraints,
    PENDING_MANUAL,
};

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
fn missing_field_is_incomparable_with_names_and_no_default() {
    let mut d = device();
    d.firmware.clear();
    let r = eval_hard_constraints(&d, "REFUSAL_TO_COOL", &HardConstraints::default());
    assert!(!r.comparable);
    assert!(r.missing.contains(&"firmware".to_string()));
    assert!(r.cluster_key.is_none());
}

#[test]
fn kernel_decision_always_carries_evidence() {
    let k = DeterministicKernel::default();
    let d = k.evaluate("不制冷", &device(), 0);
    assert_eq!(d.category, "REFUSAL_TO_COOL");
    assert!(d.constraint.comparable);
    assert!(d.evidence.iter().any(|e| e.contains("cluster_key=")));
    assert!(!d.degraded);
}

#[test]
fn out_of_vocabulary_is_not_comparable() {
    let r = eval_hard_constraints(&device(), PENDING_MANUAL, &HardConstraints::default());
    assert!(!r.comparable);
    assert!(r.missing.iter().any(|m| m.contains("fault_category")));
}

#[test]
fn time_window_is_bounded_by_configuration() {
    let cfg = HardConstraints {
        time_window_secs: 60,
        ..HardConstraints::default()
    };
    assert!(dfa_core::within_time_window(1000, 1030, &cfg));
    assert!(!dfa_core::within_time_window(1000, 2000, &cfg));
    assert_eq!(FaultVocabulary::standard().ids().len(), 6);
}
