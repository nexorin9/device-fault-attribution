//! T12 / T37 proof：就绪三值三例、不可用文案为「工单源不可用」而非「无故障」。

use dfa_ingest::{Readiness, SourceHealth, WorkOrderSource};

fn never_says_no_fault(text: &str) -> bool {
    !text.contains("无故障")
}

#[test]
fn three_readiness_states_aggregate_correctly() {
    let ready = SourceHealth {
        workorders: Readiness::Ready,
        devices: Readiness::Ready,
        teams: Readiness::Ready,
    };
    assert_eq!(ready.aggregate(), Readiness::Ready);
    assert_eq!(ready.banner(), "工单源就绪");

    let scanning = SourceHealth::default();
    assert_eq!(scanning.aggregate(), Readiness::Scanning);
    assert_eq!(scanning.banner(), "正在读取工单源…");

    let unavailable = SourceHealth {
        workorders: Readiness::Unavailable,
        devices: Readiness::Ready,
        teams: Readiness::Ready,
    };
    assert_eq!(unavailable.aggregate(), Readiness::Unavailable);
    assert_eq!(unavailable.banner(), "工单源不可用（请检查地址与令牌，稍后重试）");
}

#[test]
fn unavailable_never_reports_no_fault() {
    let health = SourceHealth {
        workorders: Readiness::Unavailable,
        devices: Readiness::Ready,
        teams: Readiness::Ready,
    };
    let texts = [
        health.banner(),
        health.empty_state_text(0),
        health.empty_state_text(12),
    ];
    for t in &texts {
        assert!(never_says_no_fault(t), "不得出现「无故障」：{t}");
    }
    assert_eq!(health.empty_state_text(0), "工单源不可用");
    assert!(health.empty_state_text(0).contains("工单源不可用"));
}

#[test]
fn ready_with_zero_items_is_the_only_real_empty_state() {
    let health = SourceHealth {
        workorders: Readiness::Ready,
        devices: Readiness::Ready,
        teams: Readiness::Ready,
    };
    assert_eq!(health.empty_state_text(0), "确实没有新工单");
    assert_eq!(health.empty_state_text(3), "当前 3 条待处理");
}

#[test]
fn workorder_source_unavailable_is_not_an_empty_order_list() {
    let mut src = WorkOrderSource::new(Vec::new());
    assert_eq!(src.readiness(), Readiness::Ready);
    assert!(src.fetch_open().unwrap().is_empty());
    src.available = false;
    let err = src.fetch_open().unwrap_err();
    assert_eq!(src.readiness(), Readiness::Unavailable);
    assert!(format!("{err:?}").contains("不得显示"));
}
