use dfa_model::{request_body, validate, OutputKind, SchemaViolation};
use serde_json::json;

#[test]
fn three_structured_output_kinds_validate() {
    assert!(validate(
        OutputKind::Merge,
        &json!({"category": "REFUSAL_TO_COOL", "evidence": "词表精确键"})
    )
    .is_ok());
    assert!(validate(
        OutputKind::SameSource,
        &json!({"verdict": "same", "evidence": {
            "model": "M", "firmware": "F", "batch": "B", "time_window": "30d"
        }})
    )
    .is_ok());
    assert!(validate(
        OutputKind::RootCause,
        &json!({"hypotheses": [{
            "text": "电源板老化", "confidence": "high",
            "evidence_refs": ["model=M", "firmware=F"]
        }]})
    )
    .is_ok());
}

#[test]
fn missing_type_and_enum_violations_have_distinct_codes() {
    let missing = validate(OutputKind::Merge, &json!({"category": "X"})).unwrap_err();
    assert_eq!(missing.code(), "ERR_SCHEMA_MISSING_FIELD");
    assert!(matches!(missing, SchemaViolation::MissingField(_)));

    let wrong_type = validate(OutputKind::Merge, &json!({"category": 1, "evidence": "e"})).unwrap_err();
    assert_eq!(wrong_type.code(), "ERR_SCHEMA_TYPE_MISMATCH");
    assert!(matches!(wrong_type, SchemaViolation::TypeMismatch(_)));

    let enum_bad = validate(
        OutputKind::SameSource,
        &json!({"verdict": "maybe", "evidence": {
            "model": "M", "firmware": "F", "batch": "B", "time_window": "30d"
        }}),
    )
    .unwrap_err();
    assert_eq!(enum_bad.code(), "ERR_SCHEMA_ENUM_OUT_OF_RANGE");
    assert!(matches!(enum_bad, SchemaViolation::EnumOutOfRange(_)));
}

#[test]
fn request_body_has_only_provider_neutral_fields() {
    let b = request_body("m", "system", "user", 30);
    let keys: Vec<&str> = b.as_object().unwrap().keys().map(|s| s.as_str()).collect();
    for k in ["model", "messages", "response_format", "temperature", "timeout"] {
        assert!(keys.contains(&k));
    }
    assert_eq!(keys.len(), 5);
    assert_eq!(b["messages"][1]["role"], "user");
    assert_eq!(OutputKind::Merge.name(), "merge_result");
}
