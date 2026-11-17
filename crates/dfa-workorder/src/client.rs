//! 真实出站写客户端：POST {WORKORDER_BASE_URL}/api/v1/work-orders。
//! 请求头 Bearer + Idempotency-Key，body 10 字段；写前校验、逐条回执、
//! 单条失败不中断同批、指数退避重放、本地提交账本幂等。
//! 生产代码路径不出现任何测试替身地址：地址只来自环境变量。

use dfa_core::{idempotency_key, Receipt, ReceiptOutcome};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::thread::sleep;
use std::time::Duration;

use crate::precheck::{precheck, PrecheckInput, WorkOrderDraft};
use crate::receipt::{backoff_ms, body_digest, ReceiptLedger, CONNECT_TIMEOUT_SECS, MAX_RETRIES};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinkRequest {
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinkResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SinkFailure {
    Timeout(String),
    Connection(String),
    Status(u16, String),
}

/// 出站通道可替换：生产用 reqwest，契约测试用本地协议替身（仅在测试代码里）。
pub trait HttpSink {
    fn post(&self, request: &SinkRequest) -> Result<SinkResponse, SinkFailure>;
}

pub struct ReqwestSink {
    client: reqwest::blocking::Client,
}

impl ReqwestSink {
    pub fn new() -> Self {
        ReqwestSink {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Default for ReqwestSink {
    fn default() -> Self {
        ReqwestSink::new()
    }
}

impl HttpSink for ReqwestSink {
    fn post(&self, request: &SinkRequest) -> Result<SinkResponse, SinkFailure> {
        let mut builder = self
            .client
            .post(&request.url)
            .timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS));
        for (k, v) in &request.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        let response = builder.json(&request.body).send().map_err(|e| {
            if e.is_timeout() {
                SinkFailure::Timeout(e.to_string())
            } else {
                SinkFailure::Connection(e.to_string())
            }
        })?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|e| SinkFailure::Connection(e.to_string()))?;
        Ok(SinkResponse { status, body })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchResult {
    pub receipts: Vec<Receipt>,
    pub rejected: Vec<String>,
    pub outbound_requests: usize,
}

pub struct WorkOrderClient<S: HttpSink> {
    pub sink: S,
    pub base_url: String,
    pub token: String,
    pub ledger: ReceiptLedger,
    pub outbound_requests: usize,
    drafts: HashMap<String, WorkOrderDraft>,
    real_backoff: bool,
}

impl<S: HttpSink> WorkOrderClient<S> {
    pub fn new(sink: S, base_url: &str, token: &str) -> Self {
        WorkOrderClient {
            sink,
            base_url: base_url.to_string(),
            token: token.to_string(),
            ledger: ReceiptLedger::new(),
            outbound_requests: 0,
            drafts: HashMap::new(),
            real_backoff: true,
        }
    }

    /// 测试可关闭真实退避等待；生产恒为 true。
    pub fn without_backoff_wait(mut self) -> Self {
        self.real_backoff = false;
        self
    }

    pub fn endpoint(&self) -> String {
        format!(
            "{}/api/v1/work-orders",
            self.base_url.trim_end_matches('/')
        )
    }

    /// 请求头与请求体按真实字段规范构造；幂等键只由 cluster_id 与工单号决定。
    pub fn build_request(&self, draft: &WorkOrderDraft) -> SinkRequest {
        let mut headers = BTreeMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", self.token),
        );
        headers.insert(
            "Idempotency-Key".to_string(),
            idempotency_key(&draft.cluster_id, &draft.workorder_no),
        );
        SinkRequest {
            url: self.endpoint(),
            headers,
            body: draft.to_body(),
        }
    }

    /// 一次确认批量建单：逐条处理，单条失败不中断同批。
    pub fn submit_batch(
        &mut self,
        drafts: &[WorkOrderDraft],
        input: &PrecheckInput,
    ) -> BatchResult {
        let mut receipts = Vec::new();
        let mut rejected = Vec::new();
        for draft in drafts {
            self.drafts
                .insert(draft.workorder_no.clone(), draft.clone());
            let key = idempotency_key(&draft.cluster_id, &draft.workorder_no);
            let check = precheck(draft, input);
            if !check.allowed {
                rejected.push(format!("{}：{}", draft.workorder_no, check.reasons.join("；")));
                receipts.push(Receipt {
                    workorder_no: draft.workorder_no.clone(),
                    cluster_id: draft.cluster_id.clone(),
                    outcome: ReceiptOutcome::Failed,
                    retryable: false,
                    http_status: 0,
                    body_digest: String::new(),
                    retry_count: 0,
                    idempotency_key: key,
                });
                continue;
            }
            // 本地提交账本：同键已成功则不产生第二张单。
            if let Some(existing) = self.ledger.get(&key) {
                if !existing.retryable {
                    receipts.push(existing.clone());
                    continue;
                }
            }
            let request = self.build_request(draft);
            let mut status = 0i64;
            let mut ok = false;
            let mut retries = 0u32;
            let mut digest = String::new();
            for attempt in 0..=MAX_RETRIES {
                retries = attempt;
                self.outbound_requests += 1;
                match self.sink.post(&request) {
                    Ok(resp) => {
                        status = resp.status as i64;
                        digest = body_digest(&resp.body);
                        if (200..300).contains(&resp.status) {
                            ok = true;
                            break;
                        }
                    }
                    Err(failure) => {
                        status = match &failure {
                            SinkFailure::Timeout(_) => 408,
                            SinkFailure::Connection(_) => 0,
                            SinkFailure::Status(code, _) => *code as i64,
                        };
                        digest = body_digest(&format!("{failure:?}"));
                    }
                }
                if attempt < MAX_RETRIES {
                    let ms = backoff_ms(attempt);
                    if self.real_backoff {
                        sleep(Duration::from_millis(ms));
                    }
                }
            }
            let receipt = Receipt {
                workorder_no: draft.workorder_no.clone(),
                cluster_id: draft.cluster_id.clone(),
                outcome: if ok {
                    ReceiptOutcome::Success
                } else {
                    ReceiptOutcome::Retryable
                },
                retryable: !ok,
                http_status: status,
                body_digest: digest,
                retry_count: retries,
                idempotency_key: key,
            };
            self.ledger.record(receipt.clone());
            receipts.push(receipt);
        }
        BatchResult {
            receipts,
            rejected,
            outbound_requests: self.outbound_requests,
        }
    }

    /// 重放：只补齐 retryable 项，复用原幂等键；已成功项不重复提交。
    pub fn replay(&mut self, cluster_id: &str) -> Vec<Receipt> {
        let pending = self.ledger.retryable_for_cluster(cluster_id);
        let mut out = Vec::new();
        for r in pending {
            let draft = match self.drafts.get(&r.workorder_no) {
                Some(d) => d.clone(),
                None => continue,
            };
            let request = self.build_request(&draft);
            self.outbound_requests += 1;
            let ok = match self.sink.post(&request) {
                Ok(resp) => (200..300).contains(&resp.status),
                Err(_) => false,
            };
            let status = if ok { 200 } else { 0 };
            self.ledger
                .mark_success(&r.idempotency_key, ok, status);
            if let Some(updated) = self.ledger.get(&r.idempotency_key) {
                out.push(updated.clone());
            }
        }
        out
    }
}
