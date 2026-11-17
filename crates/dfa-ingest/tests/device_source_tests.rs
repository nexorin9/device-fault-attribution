//! T35 proof：3 页分页遍历完整、pending_verify 设备数 = 样例预期值、
//! 游标失效只重置一次且告警计数 = 1。

use dfa_core::Device;
use dfa_ingest::{PagedDeviceSource, Readiness};

fn device(id: &str, serial: &str, model: &str, firmware: &str, batch: &str) -> Device {
    Device {
        device_id: id.into(),
        model: model.into(),
        serial: serial.into(),
        firmware: firmware.into(),
        ward: "设备科".into(),
        purchase_batch: batch.into(),
    }
}

fn three_pages() -> Vec<Vec<Device>> {
    vec![
        vec![
            device("DEV-01", "SER-01", "M-EXAMPLE", "FW-1", "LOT-A"),
            device("DEV-02", "SER-02", "M-EXAMPLE", "FW-1", "LOT-A"),
        ],
        vec![
            device("DEV-03", "SER-03", "M-EXAMPLE", "FW-2", "LOT-B"),
            device("DEV-04", "", "M-EXAMPLE", "FW-2", "LOT-B"),
        ],
        vec![device("DEV-05", "SER-05", "M-EXAMPLE", "FW-2", "LOT-B")],
    ]
}

#[test]
fn three_pages_are_traversed_without_gaps() {
    let mut src = PagedDeviceSource::new(three_pages());
    let out = src.fetch_all();
    assert_eq!(out.pages_read, 3);
    assert_eq!(out.devices.len(), 5);
    assert_eq!(out.health, Readiness::Ready);
    let ids: Vec<&str> = out.devices.iter().map(|d| d.device_id.as_str()).collect();
    assert_eq!(ids, vec!["DEV-01", "DEV-02", "DEV-03", "DEV-04", "DEV-05"]);
}

#[test]
fn devices_with_missing_identity_fields_go_to_pending_verify() {
    let raw = include_str!("../../../fixtures/devices_sample.json");
    let fixtures: Vec<Device> = serde_json::from_str(raw).unwrap();
    let expected = fixtures.iter().filter(|d| !d.missing_fields().is_empty()).count();
    assert!(fixtures.len() >= 20);

    let mut src = PagedDeviceSource::new(vec![fixtures.clone()]);
    let out = src.fetch_all();
    assert_eq!(out.pending_verify.len(), expected, "待核实设备数 = 样例预期值");
    for (id, missing) in &out.pending_verify {
        assert!(!missing.is_empty());
        assert!(fixtures.iter().any(|d| &d.device_id == id));
    }
    // 缺 serial / model 的设备不得被猜测填补。
    let missing_serial: Vec<_> = fixtures.iter().filter(|d| d.serial.trim().is_empty()).collect();
    assert!(!missing_serial.is_empty());
    assert!(out
        .pending_verify
        .iter()
        .any(|(id, m)| missing_serial.iter().any(|d| &d.device_id == id) && m.contains(&"serial".to_string())));
}

#[test]
fn invalid_cursor_resets_exactly_once_with_one_warning() {
    let mut src = PagedDeviceSource::new(three_pages());
    src.invalid_cursor_page = Some(1);
    let out = src.fetch_all();
    assert_eq!(src.warning_count, 1, "游标失效只重置一次");
    assert_eq!(
        out.warnings.iter().filter(|w| w.contains("游标失效")).count(),
        1,
        "告警计数 = 1"
    );
    assert!(out.warnings.iter().any(|w| w.contains("重置从头")));
    assert!(out.pages_read >= 3, "重置后仍要读完所有页");
}

#[test]
fn empty_last_page_terminates_and_failing_page_backs_off() {
    let mut pages = three_pages();
    pages.push(Vec::new());
    let mut src = PagedDeviceSource::new(pages);
    src.failing_page = Some(1);
    let out = src.fetch_all();
    assert_eq!(out.pages_read, 3, "末页空页正常终止，不把空页计入");
    assert_eq!(src.retry_count, 1);
    assert!(out.warnings.iter().any(|w| w.contains("5xx")));
    assert_eq!(out.devices.len(), 5);
}
