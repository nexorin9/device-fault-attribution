//! 事件扇入队列：三路源的变化进同一队列，按 (源, 事件键) 去重；
//! 同一工单跨多 tick 只入队 1 次，同批顺序按 (reported_at, workorder_no) 稳定。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestEvent {
    pub source: String,
    pub key: String,
    pub workorder_no: String,
    pub reported_epoch: i64,
}

#[derive(Debug, Clone, Default)]
pub struct EventQueue {
    seen: HashSet<(String, String)>,
    events: Vec<IngestEvent>,
    pub enqueue_calls: usize,
}

impl EventQueue {
    pub fn new() -> Self {
        EventQueue::default()
    }

    /// 返回是否真正入队；重复事件被去重，不重复消耗确认预算。
    pub fn push(&mut self, event: IngestEvent) -> bool {
        self.enqueue_calls += 1;
        let dedup_key = (event.source.clone(), event.key.clone());
        if !self.seen.insert(dedup_key) {
            return false;
        }
        self.events.push(event);
        true
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// 稳定顺序：重放同一批事件得到同一序列。
    pub fn sorted_events(&self) -> Vec<IngestEvent> {
        let mut out = self.events.clone();
        out.sort_by(|a, b| {
            a.reported_epoch
                .cmp(&b.reported_epoch)
                .then_with(|| a.workorder_no.cmp(&b.workorder_no))
                .then_with(|| a.source.cmp(&b.source))
        });
        out
    }
}
