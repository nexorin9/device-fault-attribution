//! 权威设备注册表：型号键解析成功才纳入；身份不全者进「待核实」队列，
//! 不进入任何簇、不生成工单。注册表可由源数据重建，**不提供回写院内系统的接口**。

use dfa_core::{Device, DeviceStableId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDevice {
    pub device_id: String,
    pub missing: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct Registry {
    devices: HashMap<String, Device>,
    stable_keys: HashMap<String, String>,
    pending: Vec<PendingDevice>,
}

impl Registry {
    /// 由设备台账源数据重建（唯一的构建入口）。
    pub fn build(devices: Vec<Device>) -> Registry {
        let mut registry = Registry::default();
        registry.rebuild(devices);
        registry
    }

    /// 台账补录后重新纳入：重新构建，字段齐备的设备回到正常注册表。
    pub fn rebuild(&mut self, devices: Vec<Device>) {
        self.devices.clear();
        self.stable_keys.clear();
        self.pending.clear();
        for device in devices {
            if !device.identity_complete() {
                let missing = device
                    .missing_fields()
                    .iter()
                    .filter(|f| **f == "serial" || **f == "model")
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>();
                self.pending.push(PendingDevice {
                    device_id: device.device_id.clone(),
                    missing: missing.clone(),
                    reason: format!("身份不可解析，缺字段：{}", missing.join(" / ")),
                });
                continue;
            }
            if let dfa_core::IdentityResolution::Resolved(id) = DeviceStableId::from_parts(
                &device.model,
                &device.serial,
                &device.purchase_batch,
            ) {
                self.stable_keys.insert(device.device_id.clone(), id.key);
            }
            self.devices.insert(device.device_id.clone(), device);
        }
    }

    pub fn get(&self, device_id: &str) -> Option<&Device> {
        self.devices.get(device_id)
    }

    pub fn stable_key(&self, device_id: &str) -> Option<String> {
        self.stable_keys.get(device_id).cloned()
    }

    pub fn pending(&self) -> &[PendingDevice] {
        &self.pending
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// 按缺失字段分组，供界面单列展示与补录引导。
    pub fn pending_by_missing_field(&self) -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for p in &self.pending {
            let key = if p.missing.is_empty() {
                "unknown".to_string()
            } else {
                p.missing.join("+")
            };
            out.entry(key).or_default().push(p.device_id.clone());
        }
        out
    }

    pub fn len(&self) -> usize {
        self.devices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn contains(&self, device_id: &str) -> bool {
        self.devices.contains_key(device_id)
    }
}
