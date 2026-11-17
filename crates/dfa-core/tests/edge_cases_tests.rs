use dfa_core::{clean_text, needs_verification, parse_workorder_lines, DEFAULT_MAX_FIELD_LEN};

fn cases() -> Vec<serde_json::Value> {
    let raw = include_str!("../../../fixtures/edge_cases.json");
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();
    v["cases"].as_array().unwrap().clone()
}

#[test]
fn all_edge_cases_run_without_panic() {
    let cases = cases();
    assert!(cases.len() >= 5);
    for c in &cases {
        let text = c["text"].as_str().unwrap_or("");
        let r = clean_text(text, DEFAULT_MAX_FIELD_LEN);
        assert!(r.text.chars().count() <= DEFAULT_MAX_FIELD_LEN);
        let _ = needs_verification(&r);
    }
}

#[test]
fn illegal_bytes_and_blank_values_are_marked_for_verification() {
    let illegal = clean_text("不制冷 \u{FFFD} 没反应", 4000);
    assert!(needs_verification(&illegal));
    assert!(illegal.flags.iter().any(|f| f.contains("ILLEGAL_BYTE")));
    let blank = clean_text("   \t  ", 4000);
    assert!(needs_verification(&blank));
}

#[test]
fn truncation_records_original_length_and_position() {
    let long = "不制冷".repeat(3000);
    let r = clean_text(&long, 100);
    assert!(r.truncated);
    assert_eq!(r.truncation_pos, 100);
    assert_eq!(r.original_len, 9000);
    assert_eq!(r.text.chars().count(), 100);
    assert!(r.flags.iter().any(|f| f.starts_with("TRUNCATED:9000->100")));
}

#[test]
fn dirty_lines_are_isolated_and_counted() {
    let lines = vec![
        "{\"workorder_no\":\"WO-1\",\"device_id\":\"DEV-1\",\"fault_text\":\"不制冷\"}".to_string(),
        "{ this is not json".to_string(),
        "{\"workorder_no\":\"WO-2\",\"device_id\":\"DEV-2\",\"fault_text\":\"没反应\"}".to_string(),
        "{\"workorder_no\":\"\",\"device_id\":\"\"}".to_string(),
    ];
    let (ok, failed) = parse_workorder_lines(&lines);
    assert_eq!(ok.len(), 2);
    assert_eq!(failed, 2);
}
