use dfa_core::{redact, ErrorCode, LogLine, RunSummary};
use std::collections::HashSet;

#[test]
fn log_lines_are_parseable_json_with_required_fields() {
    let line = LogLine::new("dfa-cluster", "cluster_formed", 1741000000)
        .with("cluster_id", "C-001")
        .to_json_line();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["ts"], 1741000000);
    assert_eq!(v["level"], "INFO");
    assert_eq!(v["module"], "dfa-cluster");
    assert_eq!(v["event"], "cluster_formed");
    assert_eq!(v["fields"]["cluster_id"], "C-001");
}

#[test]
fn summary_counts_match_processed_items() {
    let s = RunSummary {
        scanned: 17,
        added: 10,
        recurring: 5,
        clusters: 2,
        degradations: 1,
        outbound_writes: 0,
        failed_rows: 1,
        truncations: 1,
    };
    assert!(s.line().contains("扫描=17"));
    assert!(s.line().contains("成簇=2"));
    let v: serde_json::Value = serde_json::from_str(&s.to_log_line(1)).unwrap();
    assert_eq!(v["fields"]["scanned"], "17");
    assert_eq!(v["fields"]["outbound_writes"], "0");
}

#[test]
fn sensitive_values_never_appear_in_logs() {
    let raw = "Authorization: Bearer SECRET-TOKEN-1234567890123 api_key=ABCDEFG12345 手机 13800138000";
    let r = redact(raw);
    assert!(!r.contains("SECRET-TOKEN"));
    assert!(!r.contains("ABCDEFG12345"));
    assert!(!r.contains("13800138000"));
    assert!(r.contains("[REDACTED]"));
}

#[test]
fn eight_error_codes_are_distinct() {
    let codes = [
        ErrorCode::SourceUnavailable,
        ErrorCode::CredentialMissing,
        ErrorCode::FieldMissing,
        ErrorCode::VocabularyOutOfRange,
        ErrorCode::IdempotencyConflict,
        ErrorCode::ModelUnreachable,
        ErrorCode::IllegalStateTransition,
        ErrorCode::CapabilityMissing,
    ];
    let set: HashSet<&str> = codes.iter().map(|c| c.code()).collect();
    assert_eq!(set.len(), 8);
    assert!(codes.iter().all(|c| !c.hint().is_empty()));
}
