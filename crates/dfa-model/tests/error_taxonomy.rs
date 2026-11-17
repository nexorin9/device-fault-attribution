use dfa_model::{ModelError, ModelErrorKind};
use std::collections::HashSet;

#[test]
fn five_error_kinds_have_distinct_codes_and_hints() {
    let kinds = [
        ModelErrorKind::Timeout,
        ModelErrorKind::ConnectionFailed,
        ModelErrorKind::Server5xx,
        ModelErrorKind::Unparseable,
        ModelErrorKind::SchemaViolation,
    ];
    let codes: HashSet<&str> = kinds.iter().map(|k| k.code()).collect();
    let hints: HashSet<&str> = kinds.iter().map(|k| k.hint()).collect();
    assert_eq!(codes.len(), 5);
    assert_eq!(hints.len(), 5);
    for k in kinds {
        let e = ModelError::new(k, "detail", 1);
        assert_eq!(e.code, k.code());
        assert_eq!(e.retries, 1);
    }
}
