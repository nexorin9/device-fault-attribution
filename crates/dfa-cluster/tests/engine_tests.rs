use dfa_cluster::{measure, run, snapshot_csv, EngineInput};
use dfa_core::{Device, WorkOrder};

fn device(id: &str) -> Device {
    Device {
        device_id: id.into(),
        model: "M-EXAMPLE".into(),
        serial: format!("SER-{id}"),
        firmware: "FW-EXAMPLE".into(),
        ward: "设备科".into(),
        purchase_batch: "LOT-EXAMPLE-01".into(),
    }
}

fn order(no: &str, id: &str, text: &str, epoch: i64) -> WorkOrder {
    WorkOrder {
        workorder_no: no.into(),
        device_id: id.into(),
        fault_text: text.into(),
        reported_epoch: epoch,
        ..WorkOrder::default()
    }
}

#[test]
fn narrow_trunk_forms_exactly_one_three_device_cluster() {
    let input = EngineInput {
        workorders: vec![
            order("WO-1", "D-1", "不制冷", 100),
            order("WO-2", "D-2", "没冷气", 110),
            order("WO-3", "D-3", "制冷不给力", 120),
            order("WO-4", "D-4", "掉电", 130),
        ],
        devices: vec![device("D-1"), device("D-2"), device("D-3"), device("D-4")],
        now_epoch: 0,
        time_window_secs: 0,
    };
    let out = run(&input);
    let three = out.clusters.iter().find(|c| c.member_count() == 3);
    assert!(three.is_some(), "应恰好形成一个 3 台设备的同源簇");
    assert_eq!(out.summary.outbound_writes, 0);
    let csv = snapshot_csv(&out.clusters);
    assert_eq!(csv.lines().count(), 1 + 3);
}

#[test]
fn pending_and_incomparable_orders_never_join_a_cluster() {
    let mut bad_device = device("D-2");
    bad_device.firmware.clear();
    let input = EngineInput {
        workorders: vec![
            order("WO-1", "D-1", "不制冷", 100),
            order("WO-2", "D-2", "不制冷", 110),
            order("WO-3", "MISSING", "不制冷", 120),
        ],
        devices: vec![device("D-1"), bad_device],
        now_epoch: 0,
        time_window_secs: 0,
    };
    let out = run(&input);
    assert!(!out.incomparable.is_empty());
    assert_eq!(out.pending_verify.len(), 1);
    assert!(out.clusters.is_empty());
}

#[test]
fn scale_metrics_hold_on_a_thousand_row_stream() {
    let mut workorders = Vec::new();
    let mut devices = Vec::new();
    for i in 0..1000 {
        let id = format!("D-{:04}", i % 100);
        workorders.push(order(&format!("WO-{i:04}"), &id, "不制冷", 1000 + i as i64));
        if i < 100 {
            devices.push(device(&id));
        }
    }
    let m = measure(&EngineInput {
        workorders,
        devices,
        now_epoch: 0,
        time_window_secs: 0,
    });
    assert_eq!(m.idempotency_uniqueness, 1.0);
    assert!(m.cluster_consistency >= 0.99);
    assert_eq!(m.receipt_completeness, 1.0);
    assert!(m.single_diff_elapsed_ms <= 200.0);
}
