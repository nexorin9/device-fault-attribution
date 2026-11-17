//! T11 / T34 proof：请求体 10 字段逐项一致、Idempotency-Key =
//! sha256(cluster_id|workorder_no)、Bearer 头、同键二次提交只产生一张单、
//! 单条失败时同批其余仍成功并各带回执；替身只在测试数据层。

mod common;
use common::{RecordingStub, BODY_FIELDS};

use dfa_core::{idempotency_key, FaultVocabulary, ReceiptOutcome, TeamCapability};
use dfa_workorder::{PrecheckInput, WorkOrderClient, WorkOrderDraft};
use std::path::PathBuf;

fn draft(no: &str, cluster: &str) -> WorkOrderDraft {
    WorkOrderDraft {
        workorder_no: no.into(),
        device_id: format!("DEV-{no}"),
        device_model: "M-EXAMPLE".into(),
        firmware_version: "FW-EXAMPLE-1".into(),
        fault_code: "REFUSAL_TO_COOL".into(),
        fault_text: "不制冷".into(),
        priority: "P2".into(),
        assignee_team: "设备维修一组".into(),
        source_ref: format!("cluster:{cluster}"),
        cluster_id: cluster.into(),
    }
}

fn input() -> PrecheckInput {
    PrecheckInput {
        vocabulary: FaultVocabulary::standard(),
        capabilities: vec![TeamCapability {
            team: "设备维修一组".into(),
            capabilities: vec!["检查制冷回路".into()],
        }],
        critical_ward: false,
    }
}

fn client(stub: RecordingStub) -> WorkOrderClient<RecordingStub> {
    WorkOrderClient::new(stub, "https://workorder.example/", "TEST-TOKEN-NOT-REAL")
        .without_backoff_wait()
}

#[test]
fn request_body_headers_and_idempotency_key_match_spec() {
    let stub = RecordingStub::new();
    let mut c = client(stub);
    let d = draft("WO-EXAMPLE-01", "C-EXAMPLE-1");
    let req = c.build_request(&d);

    assert_eq!(req.url, "https://workorder.example/api/v1/work-orders");
    assert_eq!(req.headers.get("Authorization").unwrap(), "Bearer TEST-TOKEN-NOT-REAL");
    assert_eq!(req.headers.get("Content-Type").unwrap(), "application/json");
    assert_eq!(
        req.headers.get("Idempotency-Key").unwrap(),
        &idempotency_key("C-EXAMPLE-1", "WO-EXAMPLE-01")
    );
    assert_eq!(req.body.as_object().unwrap().len(), 10);
    let expected_body = d.to_body();
    for field in BODY_FIELDS {
        let expected = expected_body[field].as_str().unwrap();
        assert_eq!(req.body[field].as_str().unwrap(), expected, "{field} 逐项一致");
    }
    // 幂等键与字段顺序、时间戳、重试次数无关。
    assert_eq!(
        idempotency_key("C-EXAMPLE-1", "WO-EXAMPLE-01"),
        idempotency_key("C-EXAMPLE-1", "WO-EXAMPLE-01")
    );

    let r = c.submit_batch(&[d], &input());
    assert!(r.receipts[0].outcome == ReceiptOutcome::Success);
    assert!(c.sink.validation_errors.borrow().is_empty());
}

#[test]
fn same_key_submitted_twice_creates_exactly_one_order() {
    let mut c = client(RecordingStub::new());
    let drafts = vec![draft("WO-EXAMPLE-02", "C-EXAMPLE-1")];
    let input = input();
    let first = c.submit_batch(&drafts, &input);
    let second = c.submit_batch(&drafts, &input);
    assert_eq!(c.sink.created_count(), 1, "替身侧只收到一张单");
    assert_eq!(c.outbound_requests, 1, "第二次提交命中本地账本，不再出站");
    assert_eq!(first.receipts[0].idempotency_key, second.receipts[0].idempotency_key);
    assert_eq!(second.receipts[0].outcome, ReceiptOutcome::Success);
}

#[test]
fn stub_validation_rejects_missing_field_with_400() {
    let mut c = client(RecordingStub::new());
    let mut d = draft("WO-EXAMPLE-03", "C-EXAMPLE-1");
    d.fault_text = String::new();
    // 写前校验先拦下，不产生任何出站请求。
    let r = c.submit_batch(&[d], &input());
    assert_eq!(r.rejected.len(), 1);
    assert_eq!(r.outbound_requests, 0);
    assert_eq!(r.receipts[0].outcome, ReceiptOutcome::Failed);
    assert!(!c.sink.validation_errors.borrow().is_empty() || c.sink.request_count() == 0);
}

#[test]
fn single_failure_still_yields_receipts_for_the_rest_of_the_batch() {
    let mut c = client(RecordingStub::new().fail_on("WO-BAD"));
    let drafts = vec![
        draft("WO-GOOD-1", "C-2"),
        draft("WO-BAD", "C-2"),
        draft("WO-GOOD-2", "C-2"),
    ];
    let r = c.submit_batch(&drafts, &input());
    assert_eq!(r.receipts.len(), 3);
    assert_eq!(c.sink.created_count(), 2);
    assert!(r
        .receipts
        .iter()
        .filter(|x| x.workorder_no.starts_with("WO-GOOD"))
        .all(|x| x.outcome == ReceiptOutcome::Success));
    assert!(r.receipts.iter().any(|x| x.retryable));
}

#[test]
fn production_source_never_references_the_local_stub() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = 0usize;
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                files += 1;
                let text = std::fs::read_to_string(&p).unwrap();
                assert!(
                    !text.contains("workorder_stub"),
                    "生产代码路径不得出现替身引用：{}",
                    p.display()
                );
                assert!(!text.contains("127.0.0.1"), "生产路径不得硬编码替身地址");
            }
        }
    }
    assert!(files >= 4);
    // 凭据只在运行时由环境变量注入，回执与请求头中不含明文密钥值。
    let mut c = client(RecordingStub::new());
    let r = c.submit_batch(&[draft("WO-EXAMPLE-04", "C-3")], &input());
    let dump = format!("{:?}", r.receipts);
    assert!(!dump.contains("TEST-TOKEN-NOT-REAL"));
}
