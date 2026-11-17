//! 仅用于对比验收的基线判定器：只按工单描述的关键词分组，
//! 不做硬约束、不做确认预算、不查注册表。**产品主路径不调用本模块。**

use dfa_core::WorkOrder;
use std::collections::{BTreeMap, HashMap};

/// 取描述中的第一个命中关键词作为分组键；无命中归 "UNKNOWN"。
pub fn baseline_group_key(text: &str) -> String {
    for kw in [
        "不制冷", "没反应", "滴滴响", "输不进去", "花屏", "掉电", "报警", "故障",
    ] {
        if text.contains(kw) {
            return kw.to_string();
        }
    }
    "UNKNOWN".to_string()
}

pub fn baseline_keyword(orders: &[WorkOrder]) -> BTreeMap<String, Vec<String>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for wo in orders {
        groups
            .entry(baseline_group_key(&wo.fault_text))
            .or_default()
            .push(wo.workorder_no.clone());
    }
    groups
}

/// 本轮判定器的分组结果（由 dfa-cluster::engine 产出），供评测脚本统一计算指标。
pub fn group_ids_from_assignments(assignments: &HashMap<String, String>) -> BTreeMap<String, Vec<String>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (wo, group) in assignments {
        groups.entry(group.clone()).or_default().push(wo.clone());
    }
    for v in groups.values_mut() {
        v.sort();
    }
    groups
}
