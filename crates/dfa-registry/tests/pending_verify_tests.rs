use dfa_core::{Device, WorkOrder};
use dfa_registry::{bind_all, Registry};

fn dev(id: &str, model: &str, serial: &str, firmware: &str, batch: &str) -> Device {
    Device {
        device_id: id.into(),
        model: model.into(),
        serial: serial.into(),
        firmware: firmware.into(),
        ward: "设备科".into(),
        purchase_batch: batch.into(),
    }
}

#[test]
fn pending_queue_is_grouped_by_missing_field() {
    let reg = Registry::build(vec![
        dev("D-1", "M-1", "", "FW", "LOT"),
        dev("D-2", "", "S-2", "FW", "LOT"),
        dev("D-3", "M-1", "S-3", "", "LOT"),
        dev("D-4", "M-1", "S-4", "FW", ""),
    ]);
    let groups = reg.pending_by_missing_field();
    assert_eq!(reg.pending_count(), 2);
    assert!(groups.contains_key("serial"));
    assert!(groups.contains_key("model"));
    assert_eq!(groups["serial"], vec!["D-1".to_string()]);
}

#[test]
fn replenished_device_rejoins_registry_on_rebuild() {
    let mut reg = Registry::build(vec![dev("D-1", "M-1", "", "FW", "LOT")]);
    assert_eq!(reg.pending_count(), 1);
    reg.rebuild(vec![dev("D-1", "M-1", "S-1", "FW", "LOT")]);
    assert_eq!(reg.pending_count(), 0);
    assert!(reg.contains("D-1"));
}

#[test]
fn pending_devices_are_rejected_before_clustering() {
    let reg = Registry::build(vec![
        dev("D-1", "M-1", "", "FW", "LOT"),
        dev("D-2", "M-1", "S-2", "FW", "LOT"),
    ]);
    let orders = vec![
        WorkOrder { workorder_no: "WO-1".into(), device_id: "D-1".into(), ..WorkOrder::default() },
        WorkOrder { workorder_no: "WO-2".into(), device_id: "D-2".into(), ..WorkOrder::default() },
    ];
    let (bound, rejected) = bind_all(&reg, &orders);
    assert_eq!(bound.len(), 1);
    assert_eq!(rejected.len(), 1);
    assert_eq!(bound[0].1.device_id, "D-2");
}
