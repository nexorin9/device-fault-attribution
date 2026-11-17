use dfa_core::{Device, WorkOrder};
use dfa_registry::{bind, bind_all, Registry};

fn dev(id: &str, model: &str, serial: &str) -> Device {
    Device {
        device_id: id.into(),
        model: model.into(),
        serial: serial.into(),
        firmware: "FW-EXAMPLE".into(),
        ward: "设备科".into(),
        purchase_batch: "LOT-EXAMPLE-01".into(),
    }
}

#[test]
fn fixtures_counts_and_pending_devices_not_registered() {
    let raw = include_str!("../../../fixtures/devices_sample.json");
    let devices: Vec<Device> = serde_json::from_str(raw).unwrap();
    assert!(devices.len() >= 20);
    let reg = Registry::build(devices.clone());
    let expected_pending = devices.iter().filter(|d| !d.identity_complete()).count();
    assert_eq!(reg.pending_count(), expected_pending);
    assert!(expected_pending >= 3);
    assert_eq!(reg.len() + reg.pending_count(), devices.len());
    for p in reg.pending() {
        assert!(!reg.contains(&p.device_id));
        assert!(p.reason.contains("身份不可解析"));
    }
}

#[test]
fn workorders_on_pending_devices_are_ambiguous() {
    let reg = Registry::build(vec![dev("D-1", "M-1", ""), dev("D-2", "M-1", "S-2")]);
    let wo = WorkOrder {
        workorder_no: "WO-1".into(),
        device_id: "D-1".into(),
        reporter_dept: "重症医学科".into(),
        ..WorkOrder::default()
    };
    assert!(!bind(&reg, &wo).is_bound());
    let (_, rejected) = bind_all(&reg, &[wo]);
    assert_eq!(rejected.len(), 1);
    assert!(rejected[0].contains("待核实设备"));
}

#[test]
fn no_ward_based_identity_guessing_path() {
    let reg = Registry::build(vec![dev("D-2", "M-1", "S-2")]);
    let guessed = WorkOrder {
        workorder_no: "WO-9".into(),
        device_id: "UNKNOWN-DEVICE".into(),
        reporter_dept: "重症医学科".into(),
        fault_text: "不制冷".into(),
        ..WorkOrder::default()
    };
    assert!(matches!(
        bind(&reg, &guessed),
        dfa_registry::BindingResult::Ambiguous { .. }
    ));
}
