use dfa_core::{idempotency_key, sha256_hex, DeviceStableId, IdentityResolution};

fn resolved_key(r: IdentityResolution) -> String {
    match r {
        IdentityResolution::Resolved(id) => id.key,
        other => panic!("期望可解析，实际 {}", other.label()),
    }
}

#[test]
fn serial_missing_returns_ambiguous() {
    let r = DeviceStableId::from_parts("监护仪 M-220", "", "LOT-EXAMPLE-01");
    assert!(matches!(r, IdentityResolution::Ambiguous(_)));
    assert!(!r.is_resolved());
}

#[test]
fn model_missing_returns_ambiguous() {
    let r = DeviceStableId::from_parts("", "SER-EXAMPLE-001", "LOT-EXAMPLE-01");
    assert!(matches!(r, IdentityResolution::Ambiguous(_)));
}

#[test]
fn stable_key_is_deterministic_and_batch_scoped() {
    let a = resolved_key(DeviceStableId::from_parts("M", "S", "B"));
    let b = resolved_key(DeviceStableId::from_parts("M", "S", "B"));
    assert_eq!(a, b);
    assert_eq!(a.len(), 24);
    let c = resolved_key(DeviceStableId::from_parts("M", "S", "B2"));
    assert_ne!(a, c);
}

#[test]
fn idempotency_key_is_stable_and_content_addressed() {
    let k = idempotency_key("C-001", "WO-EXAMPLE-001");
    assert_eq!(k, idempotency_key("C-001", "WO-EXAMPLE-001"));
    assert_eq!(k, sha256_hex("C-001|WO-EXAMPLE-001"));
    assert_ne!(k, idempotency_key("C-002", "WO-EXAMPLE-001"));
}
