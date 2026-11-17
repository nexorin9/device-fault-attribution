//! 生产传输层：reqwest blocking 客户端（连接池复用 + 超时 + 状态分类）。
//! 协议层不含任何厂商 SDK；换供应商只需改 ModelConfig 的四个配置项。

use std::time::Duration;

use crate::gateway::{ChatRequest, ChatResponse, Transport, TransportFailure};

pub struct HttpTransport {
    client: reqwest::blocking::Client,
}

impl HttpTransport {
    pub fn new() -> Self {
        HttpTransport {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Default for HttpTransport {
    fn default() -> Self {
        HttpTransport::new()
    }
}

impl Transport for HttpTransport {
    fn post_chat(&self, request: &ChatRequest) -> Result<ChatResponse, TransportFailure> {
        let mut builder = self
            .client
            .post(&request.url)
            .timeout(Duration::from_secs(request.timeout_secs.max(1)));
        for (key, value) in &request.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }
        let response = builder.json(&request.body).send().map_err(|e| {
            if e.is_timeout() {
                TransportFailure::Timeout
            } else {
                TransportFailure::Connection(e.to_string())
            }
        })?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|e| TransportFailure::Body(e.to_string()))?;
        if status >= 500 {
            return Err(TransportFailure::Status(status));
        }
        Ok(ChatResponse { status, body })
    }
}
