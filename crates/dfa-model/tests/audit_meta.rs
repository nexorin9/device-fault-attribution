mod common;
use common::{config, RecordingTransport};
use dfa_model::*;

#[test]
fn timeout_retries_twice_then_degrades_with_metadata() {
    let transport = RecordingTransport::failing(TransportFailure::Timeout, 3);
    let mut gw = ModelGateway::new(config(), transport);
    gw.real_backoff = false;
    let err = gw.complete(OutputKind::Merge, "s", "u").unwrap_err();
    assert_eq!(err.kind, ModelErrorKind::Timeout);
    assert_eq!(err.retries, 2);
    assert_eq!(gw.transport.call_count.get(), 3);
    assert_eq!(gw.metas.len(), 1);
    let meta = &gw.metas[0];
    assert!(meta.degraded);
    assert!(meta.draft);
    assert_eq!(meta.retries, 2);
    assert!(meta.assert_no_secret("TEST-KEY"));
}

#[test]
fn metadata_never_contains_plaintext_key() {
    let m = CallMeta::new("example-model", "api_key=SECRET123456", "response with Bearer SECRET123456");
    assert!(m.assert_no_secret("SECRET123456"));
    assert!(!m.to_json_line().contains("SECRET123456"));
    assert_eq!(m.request_digest.len(), 64);
    assert_eq!(m.response_digest.len(), 64);
}

#[test]
fn backoff_is_exponential_with_base_200ms() {
    assert_eq!(ModelGateway::<RecordingTransport>::backoff_ms(0), 200);
    assert_eq!(ModelGateway::<RecordingTransport>::backoff_ms(1), 400);
    assert_eq!(ModelGateway::<RecordingTransport>::backoff_ms(2), 800);
    assert_eq!(MAX_RETRIES, 2);
    assert_eq!(BACKOFF_BASE_MS, 200);
}
