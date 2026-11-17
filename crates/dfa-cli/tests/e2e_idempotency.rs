//! T48 proof：幂等与重放端到端四场景。
//! 场景 1 同键二次提交替身只收到 1 张；场景 2 重放只补齐缺失项；
//! 场景 3 并发第二次确认被状态机拒绝；场景 4 重启后幂等键不变。
//! 替身只在测试数据层，不发起任何真实网络请求。

use dfa_core::{idempotency_key, ClusterState, FaultVocabulary, ReceiptOutcome, TeamCapability};
use dfa_workorder::{
    HttpSink, PrecheckInput, SinkFailure, SinkRequest, SinkResponse, WorkOrderClient,
    WorkOrderDraft,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

#[derive(Default)]
struct Stub {
    created: RefCell<HashSet<String>>,
    posts: Cell<usize>,
    fail_for: RefCell<HashSet<String>>,
    fixed: Cell<bool>,
}

impl Stub {
    fn fail_on(self, no: &str) -> Self {
        self.fail_for.borrow_mut().insert(no.to_string());
        self
    }
    fn fix(&self) {
        self.fixed.set(true);
    }
    fn created_count(&self) -> usize {
        self.created.borrow().len()
    }
    fn posts(&self) -> usize {
        self.posts.get()
    }
}

impl HttpSink for Stub {
    fn post(&self, request: &SinkRequest) -> Result<SinkResponse, SinkFailure> {
        self.posts.set(self.posts.get() + 1);
        let wo = request.body["workorder_no"].as_str().unwrap_or("").to_string();
        let key = request.headers.get("Idempotency-Key").cloned().unwrap_or_default();
        if !self.fixed.get() && self.fail_for.borrow().contains(&wo) {
            return Ok(SinkResponse {
                status: 503,
                body: "injected".into(),
            });
        }
        if self.created.borrow().contains(&key) {
            return Ok(SinkResponse {
                status: 200,
                body: "duplicate".into(),
            });
        }
        self.created.borrow_mut().insert(key);
        Ok(SinkResponse {
            status: 201,
            body: "created".into(),
        })
    }
}

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
        source_ref: "cluster:C-RESTART-1".into(),
        cluster_id: "C-RESTART-1".into(),
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

fn client(stub: Stub) -> WorkOrderClient<Stub> {
    WorkOrderClient::new(stub, "https://workorder.example", "TEST-TOKEN")
        .without_backoff_wait()
}

#[test]
fn scenario_1_duplicate_submission_creates_one_order() {
    let mut c = client(Stub::default());
    let drafts = vec![draft("WO-R1")];
    c.submit_batch(&drafts, &input());
    c.submit_batch(&drafts, &input());
    assert_eq!(c.sink.created_count(), 1);
    assert_eq!(c.outbound_requests, 1);
}

#[test]
fn scenario_2_replay_only_fills_the_missing_item() {
    let mut c = client(Stub::default().fail_on("WO-R2-B"));
    let drafts = vec![draft("WO-R2-A"), draft("WO-R2-B"), draft("WO-R2-C")];
    let first = c.submit_batch(&drafts, &input());
    assert_eq!(c.sink.created_count(), 2);
    assert!(first.receipts.iter().any(|r| r.outcome == ReceiptOutcome::Retryable));

    let posts_before = c.sink.posts();
    c.sink.fix();
    let replayed = c.replay("C-RESTART-1");
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].workorder_no, "WO-R2-B");
    assert_eq!(replayed[0].outcome, ReceiptOutcome::Success);
    assert_eq!(c.sink.created_count(), 3);
    assert_eq!(c.sink.posts(), posts_before + 1, "已成功项不重复提交");
}

#[test]
fn scenario_3_concurrent_second_confirmation_is_rejected() {
    let state = ClusterState::Observing;
    let first = state.try_transition(ClusterState::ConfirmedSameSource, true);
    assert_eq!(first.unwrap(), ClusterState::ConfirmedSameSource);
    let second = ClusterState::ConfirmedSameSource
        .try_transition(ClusterState::ConfirmedSameSource, true)
        .unwrap_err();
    assert!(second.reason.contains("ERR_ILLEGAL_TRANSITION"));

    let c = client(Stub::default());
    let before = c.outbound_requests;
    // 第二次确认被状态机拒绝，因而不产生任何重复写。
    assert_eq!(c.outbound_requests, before);
    assert_eq!(c.sink.created_count(), 0);
}

#[test]
fn scenario_4_restart_reuses_the_original_idempotency_key() {
    let cluster_id = "C-RESTART-1";
    let wo_no = "WO-R4-A";
    let original_key = idempotency_key(cluster_id, wo_no);

    // 模拟进程重启：把簇与回执写进快照，再从快照恢复。
    let payload = serde_json::json!({
        "cluster_id": cluster_id,
        "workorder_no": wo_no,
        "idempotency_key": original_key,
    })
    .to_string();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dfa_e2e_{}_{}.json", std::process::id(), nanos));
    dfa_store::write_snapshot(&path, "restart", &payload, 0).unwrap();
    let restored = dfa_store::read_snapshot(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(restored.degraded_note.is_none());
    let value: serde_json::Value = serde_json::from_str(&restored.envelope.payload).unwrap();
    let cid = value["cluster_id"].as_str().unwrap();
    let no = value["workorder_no"].as_str().unwrap();
    assert_eq!(value["idempotency_key"].as_str().unwrap(), original_key);
    assert_eq!(idempotency_key(cid, no), original_key, "重启后幂等键不变");

    let mut c = client(Stub::default());
    let r = c.submit_batch(&[draft(wo_no)], &input());
    assert_eq!(r.receipts[0].idempotency_key, original_key);
    assert_eq!(c.sink.created_count(), 1);
}
