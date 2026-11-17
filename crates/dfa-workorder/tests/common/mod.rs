//! 本地协议替身：仅存在于测试数据层，用于契约测试。
//! 按 spec.md 的 10 字段规范校验请求体与请求头，并支持幂等与失败注入。
//! 生产代码路径（crates/dfa-workorder/src）不得出现任何替身地址。

#![allow(dead_code)]
use dfa_workorder::{HttpSink, SinkFailure, SinkRequest, SinkResponse};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

pub const BODY_FIELDS: [&str; 10] = [
    "workorder_no",
    "device_id",
    "device_model",
    "firmware_version",
    "fault_code",
    "fault_text",
    "priority",
    "assignee_team",
    "source_ref",
    "cluster_id",
];

#[derive(Debug, Default)]
pub struct RecordingStub {
    pub requests: RefCell<Vec<SinkRequest>>,
    /// 真正创建成功的工单号集合（幂等语义：同键只产生一张单）。
    pub created: RefCell<Vec<String>>,
    pub keys_seen: RefCell<HashSet<String>>,
    pub fail_for: RefCell<HashSet<String>>,
    pub fixed: Cell<bool>,
    pub validation_errors: RefCell<Vec<String>>,
}

impl RecordingStub {
    pub fn new() -> Self {
        RecordingStub::default()
    }

    pub fn fail_on(self, workorder_no: &str) -> Self {
        self.fail_for.borrow_mut().insert(workorder_no.to_string());
        self
    }

    pub fn fix(&self) {
        self.fixed.set(true);
    }

    #[allow(dead_code)]
    pub fn request_count(&self) -> usize {
        self.requests.borrow().len()
    }

    pub fn created_count(&self) -> usize {
        self.created.borrow().len()
    }

    pub fn posts_for(&self, workorder_no: &str) -> usize {
        self.requests
            .borrow()
            .iter()
            .filter(|r| r.body["workorder_no"].as_str() == Some(workorder_no))
            .count()
    }
}

impl HttpSink for RecordingStub {
    fn post(&self, request: &SinkRequest) -> Result<SinkResponse, SinkFailure> {
        self.requests.borrow_mut().push(request.clone());
        for field in BODY_FIELDS {
            if request.body.get(field).is_none()
                || request.body[field].as_str().unwrap_or("").trim().is_empty()
            {
                let msg = format!("400 缺少字段 {field}");
                self.validation_errors.borrow_mut().push(msg.clone());
                return Ok(SinkResponse {
                    status: 400,
                    body: msg,
                });
            }
        }
        if !request.headers.contains_key("Idempotency-Key")
            || !request.headers.contains_key("Authorization")
        {
            let msg = "400 缺少 Idempotency-Key 或 Authorization".to_string();
            self.validation_errors.borrow_mut().push(msg.clone());
            return Ok(SinkResponse {
                status: 400,
                body: msg,
            });
        }
        let wo = request.body["workorder_no"].as_str().unwrap().to_string();
        let key = request.headers["Idempotency-Key"].clone();
        if !self.fixed.get() && self.fail_for.borrow().contains(&wo) {
            return Ok(SinkResponse {
                status: 503,
                body: "注入失败：服务暂不可用".into(),
            });
        }
        if self.keys_seen.borrow().contains(&key) {
            return Ok(SinkResponse {
                status: 200,
                body: format!("{{\"workorder_no\":\"{wo}\",\"duplicate\":true}}"),
            });
        }
        self.keys_seen.borrow_mut().insert(key);
        self.created.borrow_mut().push(wo.clone());
        Ok(SinkResponse {
            status: 201,
            body: format!("{{\"workorder_no\":\"{wo}\"}}"),
        })
    }
}
