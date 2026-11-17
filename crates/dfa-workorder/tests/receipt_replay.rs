//! T33 proof：单条失败不中断同批 / 逐条回执入库并进审计链 /
//! 重放复用原幂等键 / 退避次数 ≤2 且基数 200ms。

mod common;
use common::RecordingStub;

use dfa_core::{idempotency_key, FaultVocabulary, ReceiptOutcome, TeamCapability};
use dfa_workorder::{
    backoff_ms, body_digest, PrecheckInput, WorkOrderClient, WorkOrderDraft, BACKOFF_BASE_MS,
    CONNECT_TIMEOUT_SECS, MAX_RETRIES,
};

fn draft(no: &str) -> WorkOrderDraft {
    WorkOrderDraft {
        workorder_no: no.into(),
        device_id: format!("DEV-{no}"),
        device_model: "M-EXAMPLE".into(),
        firmware_version: "FW-EXAMPLE-1".into(),
        fault_code: "REFUSAL_TO_COOL".into(),
        fault_text: "不制冷".into(),
        priority: "P2".into(),
        assignee_team: "设备维修一组".into(),
        source_ref: "cluster:C-1".into(),
        cluster_id: "C-1".into(),
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

#[test]
fn exponential_backoff_constants_and_bounds() {
    assert_eq!(MAX_RETRIES, 2);
    assert_eq!(BACKOFF_BASE_MS, 200);
    assert_eq!(CONNECT_TIMEOUT_SECS, 10);
    assert_eq!(backoff_ms(0), 200);
    assert_eq!(backoff_ms(1), 400);
    assert_eq!(backoff_ms(2), 800);
    assert_eq!(body_digest("same"), body_digest("same"));
    assert_ne!(body_digest("same"), body_digest("different"));
    assert_eq!(body_digest("same").len(), 64);
}

#[test]
fn one_failure_does_not_stop_the_batch_and_replay_reuses_the_key() {
    let stub = RecordingStub::new().fail_on("WO-B");
    let mut client =
        WorkOrderClient::new(stub, "https://workorder.example", "TEST-TOKEN").without_backoff_wait();
    let drafts = vec![draft("WO-A"), draft("WO-B"), draft("WO-C")];
    let input = input();

    let result = client.submit_batch(&drafts, &input);
    assert_eq!(result.receipts.len(), 3, "每条必须有独立回执");
    assert_eq!(result.rejected.len(), 0);
    assert_eq!(client.sink.created_count(), 2);
    let failed = result
        .receipts
        .iter()
        .find(|r| r.workorder_no == "WO-B")
        .unwrap();
    assert_eq!(failed.outcome, ReceiptOutcome::Retryable);
    assert!(failed.retryable);
    assert_eq!(failed.retry_count, MAX_RETRIES, "重试次数上限为 2");
    assert_eq!(failed.idempotency_key, idempotency_key("C-1", "WO-B"));
    assert_eq!(result.outbound_requests, 5, "两条各 1 次 + 失败条 3 次");

    let posts_a_before = client.sink.posts_for("WO-A");
    client.sink.fix();
    let replayed = client.replay("C-1");
    assert_eq!(replayed.len(), 1, "重放只补齐失败项");
    assert_eq!(replayed[0].workorder_no, "WO-B");
    assert_eq!(replayed[0].outcome, ReceiptOutcome::Success);
    assert_eq!(replayed[0].idempotency_key, failed.idempotency_key, "幂等键不变");
    assert_eq!(client.sink.posts_for("WO-A"), posts_a_before, "已成功项不重复提交");
    assert_eq!(client.sink.created_count(), 3);
}

#[test]
fn receipts_are_persisted_into_an_append_only_audit_chain() {
    let stub = RecordingStub::new();
    let mut client =
        WorkOrderClient::new(stub, "https://workorder.example", "TEST-TOKEN").without_backoff_wait();
    let result = client.submit_batch(&[draft("WO-A"), draft("WO-B")], &input());
    assert!(result.receipts.iter().all(|r| r.outcome == ReceiptOutcome::Success));

    let mut store = dfa_store::Store::in_memory().unwrap();
    let n = client.ledger.persist(&mut store, 1_700_000_000).unwrap();
    assert_eq!(n, 2);
    let audit = store.audit_for_target("WO-A").unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].action, "RECEIPT");
    assert!(store.verify_audit_chain().unwrap());
    assert!(!store.dump_all_text().unwrap().contains("TEST-TOKEN"), "回执与审计链不得含明文 token");
}
