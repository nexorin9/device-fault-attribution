#![allow(dead_code)]
//! 本地 OpenAI-compatible 协议替身（仅在测试数据层）。
use dfa_model::{ChatRequest, ChatResponse, ModelConfig, Transport, TransportFailure};
use std::cell::{Cell, RefCell};

pub struct RecordingTransport {
    pub requests: RefCell<Vec<ChatRequest>>,
    pub failures: RefCell<Vec<TransportFailure>>,
    pub response_body: String,
    pub call_count: Cell<usize>,
}

impl RecordingTransport {
    pub fn json(body: &str) -> Self {
        RecordingTransport {
            requests: RefCell::new(Vec::new()),
            failures: RefCell::new(Vec::new()),
            response_body: body.to_string(),
            call_count: Cell::new(0),
        }
    }

    pub fn failing(failure: TransportFailure, times: usize) -> Self {
        RecordingTransport {
            requests: RefCell::new(Vec::new()),
            failures: RefCell::new((0..times).map(|_| failure.clone()).collect()),
            response_body: String::new(),
            call_count: Cell::new(0),
        }
    }
}

impl Transport for RecordingTransport {
    fn post_chat(&self, request: &ChatRequest) -> Result<ChatResponse, TransportFailure> {
        self.requests.borrow_mut().push(request.clone());
        let i = self.call_count.get();
        self.call_count.set(i + 1);
        if let Some(f) = self.failures.borrow().get(i).cloned() {
            return Err(f);
        }
        Ok(ChatResponse {
            status: 200,
            body: self.response_body.clone(),
        })
    }
}

pub fn config() -> ModelConfig {
    ModelConfig {
        base_url: "https://model.example/v1".into(),
        api_key: "TEST-KEY".into(),
        model: "example-model".into(),
        timeout_secs: 30,
    }
}

/// 真实 OpenAI-compatible 形状：choices[0].message.content 是 JSON 字符串。
pub fn openai_merge_response() -> String {
    r#"{"choices":[{"message":{"role":"assistant","content":"{\"category\":\"REFUSAL_TO_COOL\",\"evidence\":\"词表精确键\"}"}}]}"#.to_string()
}
