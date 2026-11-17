use dfa_core::{Device, DeviceStableId, WorkOrder};
use dfa_registry::{bind, BindingResult, Registry};

fn base() -> Device {
    Device {
        device_id: "DEV-EXAMPLE-01".into(),
        model: "M-EXAMPLE".into(),
        serial: "SER-EXAMPLE-01".into(),
        firmware: "FW-2.1.4-EXAMPLE".into(),
        ward: "重症医学科".into(),
        purchase_batch: "LOT-EXAMPLE-01".into(),
    }
}

#[test]
fn firmware_upgrade_and_ward_rename_keep_stable_key() {
    let a = base();
    let b = Device {
        firmware: "FW-2.2.0-EXAMPLE".into(),
        ward: "新病区".into(),
        ..a.clone()
    };
    let r1 = Registry::build(vec![a.clone()]);
    let r2 = Registry::build(vec![b]);
    assert_eq!(r1.stable_key("DEV-EXAMPLE-01"), r2.stable_key("DEV-EXAMPLE-01"));
    assert!(DeviceStableId::from_parts(&a.model, &a.serial, &a.purchase_batch).is_resolved());
}

#[test]
fn serial_missing_workorder_is_pending_and_not_bound() {
    let mut d = base();
    d.serial.clear();
    let reg = Registry::build(vec![d]);
    let wo = WorkOrder {
        workorder_no: "WO-2".into(),
        device_id: "DEV-EXAMPLE-01".into(),
        ..WorkOrder::default()
    };
    assert!(matches!(bind(&reg, &wo), BindingResult::Ambiguous { .. }));
    assert_eq!(reg.pending_count(), 1);
}

#[test]
fn model_conflict_is_rejected() {
    let reg = Registry::build(vec![base()]);
    let wo = WorkOrder {
        workorder_no: "WO-3".into(),
        device_id: "DEV-EXAMPLE-01".into(),
        model_hint: "M-DIFFERENT".into(),
        ..WorkOrder::default()
    };
    assert!(matches!(bind(&reg, &wo), BindingResult::Conflict { .. }));
}

#[test]
fn bound_result_carries_the_authoritative_device() {
    let reg = Registry::build(vec![base()]);
    let wo = WorkOrder {
        workorder_no: "WO-4".into(),
        device_id: "DEV-EXAMPLE-01".into(),
        ..WorkOrder::default()
    };
    match bind(&reg, &wo) {
        BindingResult::Bound { device, stable_key } => {
            assert_eq!(device.model, "M-EXAMPLE");
            assert!(!stable_key.is_empty());
        }
        other => panic!("期望绑定成功，实际 {}", other.label()),
    }
}
