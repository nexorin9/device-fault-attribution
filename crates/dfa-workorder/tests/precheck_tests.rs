//! T32 proof：词表外 fault_code 被拒且不新增类别 / 能力表缺项被拒 /
//! priority 越界被拒 / 10 字段缺失清单完整 / 关键岗位优先级下限。

use dfa_core::{FaultVocabulary, TeamCapability};
use dfa_workorder::{precheck, PrecheckInput, WorkOrderDraft};

fn draft() -> WorkOrderDraft {
    WorkOrderDraft {
        workorder_no: "WO-EXAMPLE-01".into(),
        device_id: "DEV-EXAMPLE-01".into(),
        device_model: "M-EXAMPLE".into(),
        firmware_version: "FW-EXAMPLE-1".into(),
        fault_code: "REFUSAL_TO_COOL".into(),
        fault_text: "不制冷".into(),
        priority: "P2".into(),
        assignee_team: "设备维修一组".into(),
        source_ref: "cluster:C-EXAMPLE-1".into(),
        cluster_id: "C-EXAMPLE-1".into(),
    }
}

fn input() -> PrecheckInput {
    PrecheckInput {
        vocabulary: FaultVocabulary::standard(),
        capabilities: vec![TeamCapability {
            team: "设备维修一组".into(),
            capabilities: vec!["检查制冷回路".into()],
        }],
        critical_ward: false,
    }
}

#[test]
fn happy_path_passes_all_ten_fields() {
    let d = draft();
    assert!(d.missing_fields().is_empty());
    let body = d.to_body();
    assert_eq!(body.as_object().unwrap().len(), 10);
    let r = precheck(&d, &input());
    assert!(r.allowed, "{:?}", r.reasons);
}

#[test]
fn out_of_vocabulary_fault_code_is_rejected_without_creating_a_category() {
    let mut d = draft();
    d.fault_code = "NEW_CATEGORY_GUESS".into();
    let r = precheck(&d, &input());
    assert!(!r.allowed);
    assert!(r.reasons.iter().any(|m| m.contains("待人工归类")));
    assert!(input().vocabulary.ids().len() == 6);
    assert!(!input().vocabulary.contains("NEW_CATEGORY_GUESS"));
}

#[test]
fn missing_capability_table_entry_forbids_order_creation() {
    let mut i = input();
    i.capabilities.clear();
    let r = precheck(&draft(), &i);
    assert!(!r.allowed);
    assert!(r.reasons.iter().any(|m| m.contains("取不到即禁止建单")));
}

#[test]
fn priority_out_of_range_and_critical_ward_lower_bound() {
    let mut d = draft();
    d.priority = "P9".into();
    assert!(!precheck(&d, &input()).allowed);

    d.priority = "P3".into();
    let mut i = input();
    i.critical_ward = true;
    let r = precheck(&d, &i);
    assert!(!r.allowed);
    assert!(r.reasons.iter().any(|m| m.contains("不得低于 P2")));
}

#[test]
fn missing_fields_list_covers_all_ten() {
    let d = WorkOrderDraft::default();
    assert_eq!(d.missing_fields().len(), 10);
    let r = precheck(&d, &input());
    assert_eq!(r.missing_fields.len(), 10);
    assert!(r.reasons.iter().any(|m| m.contains("缺字段")));
}
