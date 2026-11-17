//! T27 proof：关键岗位优先级更高 / 能力表缺项被剔除且有记录 / 同输入两次排序一致。

use dfa_attribution::{capability_available, order_steps, Step};
use dfa_core::TeamCapability;

fn teams() -> Vec<TeamCapability> {
    vec![
        TeamCapability {
            team: "设备维修一组".into(),
            capabilities: vec!["查压力表".into(), "换保险丝".into()],
        },
        TeamCapability {
            team: "厂商支持组".into(),
            capabilities: vec!["刷固件".into()],
        },
    ]
}

fn step(action: &str, cap: &str, rank: u32, critical: bool) -> Step {
    Step {
        action: action.into(),
        expected: "观察是否达到阈值".into(),
        criterion: "未达阈值即证伪该假设".into(),
        reason: format!("可证伪优先序 {rank}，关键岗位 = {critical}"),
        falsifiable_rank: rank,
        critical_ward: critical,
        capability: cap.into(),
    }
}

#[test]
fn critical_ward_outranks_falsifiable_rank() {
    let steps = vec![
        step("常规查压力表", "查压力表", 1, false),
        step("监护设备优先换保险丝", "换保险丝", 9, true),
    ];
    let r = order_steps(steps, &teams());
    assert_eq!(r.actions(), vec!["监护设备优先换保险丝", "常规查压力表"]);
    assert!(r.steps[0].reason.contains("关键岗位"));
    assert!(!r.steps[0].criterion.is_empty(), "每步需给出可证伪观察标准");
}

#[test]
fn missing_capability_items_are_removed_with_records() {
    let steps = vec![
        step("查压力表", "查压力表", 1, false),
        step("返厂换主板", "返厂换主板", 2, false),
        step("刷固件", "刷固件", 3, false),
    ];
    let r = order_steps(steps, &teams());
    assert_eq!(r.actions(), vec!["查压力表", "刷固件"]);
    assert_eq!(r.removed.len(), 1);
    assert_eq!(r.removed[0].0, "返厂换主板");
    assert!(r.removed[0].1.contains("已剔除"));
    assert!(capability_available(&teams(), "刷固件"));
    assert!(!capability_available(&teams(), "返厂换主板"));
}

#[test]
fn same_input_produces_identical_order_twice() {
    let build = || {
        vec![
            step("乙-刷固件", "刷固件", 2, false),
            step("甲-查压力表", "查压力表", 3, false),
            step("丙-换保险丝", "换保险丝", 1, true),
        ]
    };
    let a = order_steps(build(), &teams());
    let b = order_steps(build(), &teams());
    assert_eq!(a.actions(), b.actions());
    assert_eq!(a.removed, b.removed);
}
