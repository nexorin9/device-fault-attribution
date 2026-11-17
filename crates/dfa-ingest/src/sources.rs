//! 三路入站源：设备台账（分页 / 游标 + 字段缺失容忍）、故障工单、
//! 班组能力（每次建单前重取，取不到即禁止建单）。真实 HTTP 走 Bearer 凭据。

use dfa_core::{Device, TeamCapability, WorkOrder};
use serde::{Deserialize, Serialize};
use std::cell::Cell;

use crate::readiness::Readiness;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceError {
    Unavailable(String),
    Timeout(String),
    InvalidCursor(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFetch {
    pub devices: Vec<Device>,
    /// 缺 serial / model / batch 的设备编号 → 待核实原因。
    pub pending_verify: Vec<(String, Vec<String>)>,
    pub pages_read: usize,
    pub warnings: Vec<String>,
    pub health: Readiness,
}

/// 台账分页 / 游标遍历：末页空页终止、中途 500 退避重试、游标失效重置一次并告警。
#[derive(Debug, Clone)]
pub struct PagedDeviceSource {
    pub pages: Vec<Vec<Device>>,
    /// 指定页返回 5xx（模拟中途失败）。
    pub failing_page: Option<usize>,
    /// 指定页游标失效（模拟需要重置从头）。
    pub invalid_cursor_page: Option<usize>,
    pub warning_count: usize,
    pub retry_count: usize,
}

impl Default for PagedDeviceSource {
    fn default() -> Self {
        PagedDeviceSource {
            pages: Vec::new(),
            failing_page: None,
            invalid_cursor_page: None,
            warning_count: 0,
            retry_count: 0,
        }
    }
}

impl PagedDeviceSource {
    pub fn new(pages: Vec<Vec<Device>>) -> Self {
        PagedDeviceSource {
            pages,
            ..PagedDeviceSource::default()
        }
    }

    pub fn fetch_all(&mut self) -> DeviceFetch {
        let mut out = DeviceFetch {
            devices: Vec::new(),
            pending_verify: Vec::new(),
            pages_read: 0,
            warnings: Vec::new(),
            health: Readiness::Ready,
        };
        let mut index = 0usize;
        let mut restarted = false;
        while index < self.pages.len() {
            if self.failing_page == Some(index) {
                self.retry_count += 1;
                out.warnings.push(format!("第 {index} 页 5xx，按退避重试后继续"));
            }
            if self.invalid_cursor_page == Some(index) && !restarted {
                restarted = true;
                self.warning_count += 1;
                out.warnings.push("游标失效：重置从头一次".to_string());
                out.devices.clear();
                out.pending_verify.clear();
                index = 0;
                continue;
            }
            let page = &self.pages[index];
            if page.is_empty() {
                break;
            }
            for device in page {
                let missing_identity: Vec<String> = device
                    .missing_fields()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                if !missing_identity.is_empty() {
                    out.pending_verify
                        .push((device.device_id.clone(), missing_identity));
                }
                out.devices.push(device.clone());
            }
            out.pages_read += 1;
            index += 1;
        }
        out
    }
}

/// 工单源：不可用 → 就绪态 Unknown，不返回「无故障」。
#[derive(Debug, Clone)]
pub struct WorkOrderSource {
    pub orders: Vec<WorkOrder>,
    pub available: bool,
}

impl WorkOrderSource {
    pub fn new(orders: Vec<WorkOrder>) -> Self {
        WorkOrderSource {
            orders,
            available: true,
        }
    }

    pub fn fetch_open(&self) -> Result<Vec<WorkOrder>, SourceError> {
        if !self.available {
            return Err(SourceError::Unavailable(
                "工单源不可用：不得显示「无故障」".to_string(),
            ));
        }
        Ok(self.orders.clone())
    }

    pub fn readiness(&self) -> Readiness {
        if self.available {
            Readiness::Ready
        } else {
            Readiness::Unavailable
        }
    }
}

/// 班组能力源：每次建单前重取，绝不缓存过期能力；取不到即禁止建单。
#[derive(Debug, Clone)]
pub struct TeamCapabilitySource {
    pub teams: Vec<TeamCapability>,
    pub available: bool,
    pub request_count: Cell<usize>,
    pub outbound_writes: Cell<usize>,
}

impl TeamCapabilitySource {
    pub fn new(teams: Vec<TeamCapability>) -> Self {
        TeamCapabilitySource {
            teams,
            available: true,
            request_count: Cell::new(0),
            outbound_writes: Cell::new(0),
        }
    }

    /// 每次调用都记一次请求，测试据此断言「两次建单之间请求计数递增」。
    pub fn fetch(&self, capability: &str) -> Result<Vec<TeamCapability>, SourceError> {
        self.request_count.set(self.request_count.get() + 1);
        if !self.available {
            return Err(SourceError::Unavailable(format!(
                "能力源不可取：{capability}，禁止建单，出站写请求计数 = 0"
            )));
        }
        Ok(self
            .teams
            .iter()
            .filter(|t| t.can(capability))
            .cloned()
            .collect())
    }

    pub fn readiness(&self) -> Readiness {
        if self.available {
            Readiness::Ready
        } else {
            Readiness::Unavailable
        }
    }
}

/// 真实 HTTP GET（Bearer 凭据来自环境变量），供 doctor 与接入后的轮询使用。
pub fn http_get(base_url: &str, path: &str, token: &str) -> Result<String, SourceError> {
    if base_url.trim().is_empty() {
        return Err(SourceError::Unavailable("未配置基地址".to_string()));
    }
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .bearer_auth(token)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                SourceError::Timeout(e.to_string())
            } else {
                SourceError::Unavailable(e.to_string())
            }
        })?;
    if !response.status().is_success() {
        return Err(SourceError::Unavailable(format!(
            "HTTP {}",
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| SourceError::Unavailable(e.to_string()))
}
