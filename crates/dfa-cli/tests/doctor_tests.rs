//! T16 proof（缺凭据）：缺 WORKORDER_TOKEN 时返回可区分错误码，
//! 并明确指出缺哪个变量、不打印任何密钥值。
//! 本文件是独立测试二进制，单独控制环境变量，避免与其它测试并行冲突。

use dfa_cli::{doctor_report, ENV_KEYS};

#[test]
fn missing_credentials_return_a_distinct_exit_code_without_leaking_values() {
    let saved: Vec<(&'static str, Option<String>)> = ENV_KEYS
        .iter()
        .map(|k| (*k, std::env::var(k).ok()))
        .collect();
    for (k, _) in &saved {
        std::env::remove_var(k);
    }
    let (text, code) = doctor_report();
    for (k, v) in saved {
        match v {
            Some(value) => std::env::set_var(k, value),
            None => std::env::remove_var(k),
        }
    }

    assert_eq!(code, 3, "缺凭据必须返回可区分错误码，而非静默通过");
    assert!(text.contains("ERR_CREDENTIAL_MISSING"));
    assert!(text.contains("WORKORDER_TOKEN"), "明确指出缺哪个变量");
    assert!(text.contains(".env.example"), "给出取值位置");
    assert!(!text.contains("SECRET-TOKEN-VALUE"), "不得打印任何密钥值");
    assert!(text.contains("检查项 | 结论 | 建议动作"));
}
