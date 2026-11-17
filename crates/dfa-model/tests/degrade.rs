mod common;
use common::{config, RecordingTransport};
use dfa_core::{ClusterState, DEGRADE_LABEL};
use dfa_model::*;

#[test]
fn model_unreachable_degrades_with_label_and_never_advances_state() {
    let transport = RecordingTransport::failing(TransportFailure::Connection("down".into()), 3);
    let mut gw = ModelGateway::new(config(), transport);
    gw.real_backoff = false;
    let err = gw.complete(OutputKind::Merge, "s", "u").unwrap_err();
    let out = degrade("REFUSAL_TO_COOL", &err);
    assert!(out.degraded);
    assert_eq!(out.label, DEGRADE_LABEL);
    assert_eq!(out.value, "REFUSAL_TO_COOL");
    assert_eq!(out.retries, 2);
    assert_eq!(out.error_code, "ERR_MODEL_UNREACHABLE");
    let before = ClusterState::Observing;
    let after = before.transition_or_self(ClusterState::ConfirmedSameSource, false);
    assert_eq!(after, before);
}

#[test]
fn success_is_labelled_as_draft() {
    let out = success("value");
    assert!(!out.degraded);
    assert!(out.label.contains("草稿"));
}

#[test]
fn parse_content_supports_both_openai_shape_and_plain_json() {
    let v = parse_content(&common::openai_merge_response()).unwrap();
    assert_eq!(v["category"], "REFUSAL_TO_COOL");
    let plain = parse_content(r#"{"category":"NO_RESPONSE","evidence":"e"}"#).unwrap();
    assert_eq!(plain["category"], "NO_RESPONSE");
}
