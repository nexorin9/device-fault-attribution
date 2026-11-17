//! 可审计调用元数据：model / 请求摘要 / 响应摘要 / 耗时 / 重试次数 / 是否降级 /
//! 是否为草稿。api_key 不入库、不进日志。

use dfa_core::{redact, sha256_hex};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallMeta {
    pub model: String,
    pub request_digest: String,
    pub response_digest: String,
    pub elapsed_ms: u128,
    pub retries: u32,
    pub degraded: bool,
    pub draft: bool,
}

impl CallMeta {
    pub fn new(model: &str, request: &str, response: &str) -> Self {
        CallMeta {
            model: redact(model),
            request_digest: sha256_hex(&redact(request)),
            response_digest: sha256_hex(&redact(response)),
            elapsed_ms: 0,
            retries: 0,
            degraded: false,
            draft: true,
        }
    }

    /// 落库前断言无明文密钥；任何敏感片段都已被遮蔽。
    pub fn assert_no_secret(&self, secret: &str) -> bool {
        if secret.is_empty() {
            return true;
        }
        !self.request_digest.contains(secret)
            && !self.response_digest.contains(secret)
            && !self.model.contains(secret)
            && !format!("{}", self.elapsed_ms).contains(secret)
    }

    pub fn to_json_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}
