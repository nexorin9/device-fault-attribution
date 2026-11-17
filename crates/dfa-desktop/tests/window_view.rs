//! T14 proof：窗口进程零出站请求、簇卡片字段与快照内容逐项一致、两栏联动。

use dfa_core::{Cluster, ClusterMember};
use dfa_desktop::{batch_confirm_preview, BatchConfirmInput, PendingRow, SnapshotView};

fn member(wo: &str, dev: &str) -> ClusterMember {
    ClusterMember {
        workorder_no: wo.into(),
        device_id: dev.into(),
        model: "M-EXAMPLE".into(),
        firmware: "FW-EXAMPLE-1".into(),
        purchase_batch: "<EXAMPLE_LOT>".into(),
        ward: "重症医学科".into(),
        fault_text: "不制冷".into(),
        reported_at: "2026-01-01 08:00".into(),
        reported_epoch: 1_700_000_000,
        fault_category: "REFUSAL_TO_COOL".into(),
    }
}

fn view() -> SnapshotView {
    let mut c = Cluster::observing("C-EXAMPLE-1", "M-EXAMPLE|FW-EXAMPLE-1|<EXAMPLE_LOT>", "REFUSAL_TO_COOL");
    c.members = vec![member("WO-1", "DEV-1"), member("WO-2", "DEV-2")];
    c.evidence = vec!["model=M-EXAMPLE".into()];
    c.hypothesis = vec!["制冷回路泄漏（草稿）".into()];
    c.steps = vec!["先查压力表".into()];
    let mut v = SnapshotView::from_clusters(&[c], "就绪");
    v.pending_rows = vec![PendingRow {
        device_id: "DEV-3".into(),
        missing: vec!["serial".into(), "purchase_batch".into()],
    }];
    v
}

#[test]
fn window_is_a_pure_snapshot_consumer_with_zero_outbound_requests() {
    let v = view();
    assert_eq!(v.outbound_requests(), 0);
    assert_eq!(v.pending_count(), 1);
    assert!(v.text_projection().contains("出站请求 0"));
    let json = serde_json::to_string(&v).unwrap();
    let restored = SnapshotView::from_snapshot_json(&json).unwrap();
    assert_eq!(restored, v, "快照往返一致，窗口只读快照文件");
}

#[test]
fn cluster_card_fields_match_snapshot_content() {
    let v = view();
    let card = &v.cluster_cards[0];
    assert_eq!(card.cluster_id, "C-EXAMPLE-1");
    assert_eq!(card.state, "观察中");
    assert_eq!(card.member_rows.len(), 2);
    let row = &card.member_rows[0];
    assert_eq!(row.device_id, "DEV-1");
    assert_eq!(row.model, "M-EXAMPLE");
    assert_eq!(row.firmware, "FW-EXAMPLE-1");
    assert_eq!(row.purchase_batch, "<EXAMPLE_LOT>");
    assert_eq!(row.fault_text, "不制冷");
    assert!(card.draft_label.contains("草稿"));
    assert_eq!(card.evidence, vec!["model=M-EXAMPLE".to_string()]);
    assert_eq!(card.hypothesis.len(), 1);
    assert_eq!(card.steps, vec!["先查压力表".to_string()]);
}

#[test]
fn two_column_layout_and_linkage_are_consistent() {
    let v = view();
    let layout = v.two_column_layout();
    assert_eq!(layout.left_title, "设备注册表");
    assert_eq!(layout.left_rows.len(), 2);
    assert_eq!(layout.left_pending.len(), 1);
    assert_eq!(layout.right_title, "故障簇");
    assert_eq!(layout.right_cards.len(), 1);
    assert_eq!(v.clusters_for_device("DEV-2"), vec!["C-EXAMPLE-1".to_string()]);
    assert_eq!(v.devices_for_cluster("C-EXAMPLE-1"), vec!["DEV-1", "DEV-2"]);
    assert_eq!(v.missing_for("DEV-3"), vec!["serial", "purchase_batch"]);
}

#[test]
fn batch_confirm_requires_second_confirmation_and_selection() {
    let input = BatchConfirmInput {
        selected_members: vec!["DEV-1".into(), "DEV-2".into()],
        category: "REFUSAL_TO_COOL".into(),
        priority: "P2".into(),
        team: "设备维修一组".into(),
        second_confirm: false,
    };
    let preview = batch_confirm_preview(&input);
    assert_eq!(preview.order_count, 2);
    assert!(!preview.ready);
    assert_eq!(preview.cancelled_outbound, 0);
    let mut ok = input.clone();
    ok.second_confirm = true;
    assert!(batch_confirm_preview(&ok).ready);
}
