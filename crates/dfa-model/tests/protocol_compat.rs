mod common;
use common::{config, openai_merge_response, RecordingTransport};
use dfa_model::*;

#[test]
fn request_shape_and_headers_match_openai_compatible() {
    let transport = RecordingTransport::json(&openai_merge_response());
    let mut gw = ModelGateway::new(config(), transport);
    gw.real_backoff = false;
    let value = gw.complete(OutputKind::Merge, "系统提示", "科室口语").unwrap();
    assert_eq!(value["category"], "REFUSAL_TO_COOL");
    let requests = gw.transport.requests.borrow();
    let req = &requests[0];
    assert_eq!(req.url, "https://model.example/v1/chat/completions");
    assert_eq!(req.headers.get("Authorization").unwrap(), "Bearer TEST-KEY");
    assert_eq!(req.headers.get("Content-Type").unwrap(), "application/json");
    assert_eq!(req.body["model"], "example-model");
    assert_eq!(req.body["response_format"]["type"], "json_object");
    assert_eq!(req.body["temperature"], 0);
    assert_eq!(req.body["timeout"], 30);
    assert_eq!(req.body["messages"].as_array().unwrap().len(), 2);
    assert_eq!(req.body["messages"][0]["role"], "system");
    assert_eq!(req.body["messages"][1]["content"], "科室口语");
}

#[test]
fn four_failure_classes_are_distinct() {
    let cases = [
        (TransportFailure::Timeout, ModelErrorKind::Timeout),
        (TransportFailure::Connection("down".into()), ModelErrorKind::ConnectionFailed),
        (TransportFailure::Status(503), ModelErrorKind::Server5xx),
        (TransportFailure::Body("broken".into()), ModelErrorKind::Unparseable),
    ];
    for (failure, expected) in cases {
        let mut gw = ModelGateway::new(config(), RecordingTransport::failing(failure, 3));
        gw.real_backoff = false;
        let err = gw.complete(OutputKind::Merge, "s", "u").unwrap_err();
        assert_eq!(err.kind, expected);
        assert_eq!(err.code, expected.code());
    }
}

#[test]
fn unparseable_content_is_a_distinct_error() {
    let mut gw = ModelGateway::new(config(), RecordingTransport::json("not-json-at-all"));
    gw.real_backoff = false;
    let err = gw.complete(OutputKind::Merge, "s", "u").unwrap_err();
    assert_eq!(err.kind, ModelErrorKind::Unparseable);
}

#[test]
fn schema_violation_is_distinct_from_unparseable() {
    let mut gw = ModelGateway::new(config(), RecordingTransport::json(r#"{"category":"X"}"#));
    gw.real_backoff = false;
    let err = gw.complete(OutputKind::Merge, "s", "u").unwrap_err();
    assert_eq!(err.kind, ModelErrorKind::SchemaViolation);
}

#[test]
fn model_config_is_fully_configuration_driven() {
    let cfg = config();
    assert_eq!(cfg.chat_completions_url(), "https://model.example/v1/chat/completions");
    assert!(cfg.configured());
    let body = request_body(&cfg.model, "s", "u", cfg.timeout_secs);
    assert_eq!(body["model"], "example-model");
    assert_eq!(body["response_format"]["type"], "json_object");
}
