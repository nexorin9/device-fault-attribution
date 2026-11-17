use dfa_core::{
    assert_no_real_identifiers, export_csv, export_markdown, mask_identifiers, Cluster,
    ClusterMember, ClusterState, CSV_HEADER, DRAFT_LABEL,
};

fn cluster(id: &str, state: ClusterState, n: usize) -> Cluster {
    let mut c = Cluster::observing(id, "M-EXAMPLE|FW-EXAMPLE|LOT-EXAMPLE-01|CAT", "REFUSAL_TO_COOL");
    c.state = state;
    c.first_seen_at = "2025-03-03".to_string();
    c.evidence.push("model=M-EXAMPLE".to_string());
    c.hypothesis.push("假设草稿".to_string());
    c.steps.push("先查电源板".to_string());
    for i in 0..n {
        c.members.push(ClusterMember {
            device_id: format!("DEV-EXAMPLE-{i:02}"),
            model: "M-EXAMPLE".to_string(),
            firmware: "FW-EXAMPLE".to_string(),
            purchase_batch: "LOT-EXAMPLE-01".to_string(),
            ward: "设备科".to_string(),
            workorder_no: format!("WO-EXAMPLE-{i:03}"),
            ..ClusterMember::default()
        });
    }
    c
}

#[test]
fn csv_row_count_equals_sum_of_exported_members() {
    let clusters = vec![
        cluster("C-1", ClusterState::ConfirmedSameSource, 3),
        cluster("C-2", ClusterState::Ordered, 2),
    ];
    let csv = export_csv(&clusters);
    assert!(csv.starts_with(CSV_HEADER));
    assert_eq!(csv.lines().count(), 1 + 5);
}

#[test]
fn observing_clusters_never_appear_in_export() {
    let clusters = vec![
        cluster("C-1", ClusterState::Observing, 2),
        cluster("C-2", ClusterState::Closed, 1),
    ];
    let csv = export_csv(&clusters);
    assert_eq!(csv.lines().count(), 2);
    assert!(!csv.contains("C-1"));
}

#[test]
fn markdown_marks_hypotheses_as_draft() {
    let clusters = vec![cluster("C-1", ClusterState::ConfirmedSameSource, 2)];
    let md = export_markdown(&clusters);
    assert!(md.contains(DRAFT_LABEL));
    assert!(md.contains("核查步骤"));
}

#[test]
fn real_style_identifiers_are_rejected_and_maskable() {
    let clusters = vec![cluster("C-1", ClusterState::ConfirmedSameSource, 2)];
    assert!(assert_no_real_identifiers(&export_csv(&clusters)).is_ok());
    assert!(assert_no_real_identifiers("B2023Q4-07").is_err());
    assert!(assert_no_real_identifiers("SN1234567890").is_err());
    assert!(assert_no_real_identifiers("13800138000").is_err());
    assert!(mask_identifiers("B2023Q4-07").contains("<EXAMPLE_LOT>"));
}
