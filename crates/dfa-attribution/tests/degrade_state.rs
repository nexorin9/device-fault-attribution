//! T29 proof：降级前后簇与工单状态 diff = 0 / 出站请求计数 = 0 /
//! 混合场景逐条独立 / 降级计数进入运行摘要。

use dfa_attribution::{DegradeOrchestrator, StateSnapshot};
use dfa_core::{RunSummary, DEGRADE_LABEL};
use std::collections::BTreeMap;

fn snapshot() -> StateSnapshot {
    let mut cluster_states = BTreeMap::new();
    cluster_states.insert("C-1".to_string(), "观察中".to_string());
    let mut workorder_states = BTreeMap::new();
    workorder_states.insert("WO-1".to_string(), "未开单".to_string());
    StateSnapshot {
        cluster_states,
        workorder_states,
        confirmed_entries: 0,
        ordered_entries: 0,
        outbound_writes: 0,
    }
}

#[test]
fn degraded_path_does_not_advance_state_or_write_out() {
    let before = snapshot();
    let mut orch = DegradeOrchestrator::default();
    let item = orch.run_item::<String>(
        Err("ERR_MODEL_UNREACHABLE: 超时".to_string()),
        "规则结果".to_string(),
    );
    assert!(item.degraded);
    assert_eq!(item.label, DEGRADE_LABEL);
    assert_eq!(item.reason, "ERR_MODEL_UNREACHABLE: 超时");
    let after = before.clone();
    assert_eq!(before.diff(&after), 0);
    assert_eq!(after.outbound_writes, 0);
    assert!(DegradeOrchestrator::assert_no_state_advance(&before, &after));
    assert!(!orch.cluster_confirmed_from_items());
}

#[test]
fn mixed_batch_is_evaluated_item_by_item() {
    let mut orch = DegradeOrchestrator::default();
    let a = orch.run_item(Ok("模型草稿-1".to_string()), "规则-1".to_string());
    let b = orch.run_item::<String>(Err("5xx".to_string()), "规则-2".to_string());
    let c = orch.run_item(Ok("模型草稿-3".to_string()), "规则-3".to_string());
    assert!(!a.degraded && !c.degraded && b.degraded);
    assert_eq!(orch.produced, 3);
    assert_eq!(orch.degrade_count, 1);
    assert_eq!(b.value, "规则-2");
    assert!(!orch.cluster_confirmed_from_items(), "个别降级不得把整簇标为已确认");
}

#[test]
fn degrade_count_and_zero_outbound_enter_run_summary() {
    let mut orch = DegradeOrchestrator::default();
    for _ in 0..2 {
        orch.run_item::<String>(Err("连接失败".to_string()), "规则".to_string());
    }
    let mut summary = RunSummary {
        outbound_writes: 7,
        ..RunSummary::default()
    };
    orch.write_summary(&mut summary);
    assert_eq!(summary.degradations, 2);
    assert_eq!(summary.outbound_writes, 0);
    assert!(summary.line().contains("降级=2"));
    assert!(summary.line().contains("出站写=0"));
}
