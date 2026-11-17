use dfa_cluster::idempotency_key;

#[test]
fn same_pair_always_yields_the_same_key() {
    let a = idempotency_key("C-001", "WO-EXAMPLE-001");
    for _ in 0..5 {
        assert_eq!(a, idempotency_key("C-001", "WO-EXAMPLE-001"));
    }
    assert_eq!(a.len(), 64);
}

#[test]
fn key_is_insensitive_to_timestamp_and_retry_count() {
    // 键只由 cluster_id 与 workorder_no 决定；时间戳 / 重试次数不是输入。
    let base = idempotency_key("C-001", "WO-EXAMPLE-001");
    let later = idempotency_key("C-001", "WO-EXAMPLE-001");
    assert_eq!(base, later);
    assert_ne!(base, idempotency_key("C-001", "WO-EXAMPLE-002"));
    assert_ne!(base, idempotency_key("C-002", "WO-EXAMPLE-001"));
}
