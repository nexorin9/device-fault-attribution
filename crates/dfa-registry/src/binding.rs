//! 工单 → 设备绑定：以「序列号 + 型号」双因子为准，绝不按科室或名称猜身份。

use dfa_core::{Device, DeviceStableId, IdentityResolution, WorkOrder};
use serde::{Deserialize, Serialize};

use crate::registry::Registry;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingResult {
    Bound {
        device: Device,
        stable_key: String,
    },
    /// 设备缺序列号 / 型号，或设备不在注册表（待核实）→ 该工单不入簇、不建单。
    Ambiguous {
        reason: String,
    },
    Conflict {
        reason: String,
    },
}

impl BindingResult {
    pub fn is_bound(&self) -> bool {
        matches!(self, BindingResult::Bound { .. })
    }

    pub fn label(&self) -> &'static str {
        match self {
            BindingResult::Bound { .. } => "已绑定",
            BindingResult::Ambiguous { .. } => "待核实设备",
            BindingResult::Conflict { .. } => "型号冲突",
        }
    }
}

pub fn bind(registry: &Registry, workorder: &WorkOrder) -> BindingResult {
    let device = match registry.get(&workorder.device_id) {
        Some(d) => d.clone(),
        None => {
            return BindingResult::Ambiguous {
                reason: format!(
                    "设备 {} 不在注册表（待核实），工单 {} 不参与同源判定",
                    workorder.device_id, workorder.workorder_no
                ),
            }
        }
    };
    if !device.identity_complete() {
        return BindingResult::Ambiguous {
            reason: format!(
                "设备 {} 序列号 / 型号不齐，工单 {} 标待核实设备",
                workorder.device_id, workorder.workorder_no
            ),
        };
    }
    if !workorder.model_hint.trim().is_empty()
        && workorder.model_hint.trim() != device.model.trim()
    {
        return BindingResult::Conflict {
            reason: format!(
                "工单 {} 携带型号 {} 与注册表型号 {} 冲突",
                workorder.workorder_no, workorder.model_hint, device.model
            ),
        };
    }
    match DeviceStableId::from_parts(&device.model, &device.serial, &device.purchase_batch) {
        IdentityResolution::Resolved(id) => BindingResult::Bound {
            device,
            stable_key: id.key,
        },
        other => BindingResult::Ambiguous {
            reason: format!("身份解析结果 {}，拒绝绑定", other.label()),
        },
    }
}

/// 批量绑定：返回可参与判定的工单与设备，以及被歧义拒绝的工单号清单。
pub fn bind_all(
    registry: &Registry,
    workorders: &[WorkOrder],
) -> (Vec<(WorkOrder, Device, String)>, Vec<String>) {
    let mut bound = Vec::new();
    let mut rejected = Vec::new();
    for wo in workorders {
        match bind(registry, wo) {
            BindingResult::Bound { device, stable_key } => {
                bound.push((wo.clone(), device, stable_key))
            }
            other => rejected.push(format!("{}:{}", wo.workorder_no, other.label())),
        }
    }
    (bound, rejected)
}
