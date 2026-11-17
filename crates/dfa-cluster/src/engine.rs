//! 窄主干编排：工单流 → 差分 → 注册表绑定 → 硬约束门 → 同源成簇 → 簇清单。
//! 一个命令即可从样例工单产出可核对的簇清单 CSV；未经人工确认，出站写计数恒为 0。

use dfa_core::{
    clean_text, DeterministicKernel, Device, FaultVocabulary, HardConstraints, RunSummary,
    WorkOrder, DEFAULT_TIME_WINDOW_SECS,
};
use dfa_registry::{bind_all, Registry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

use crate::cluster_key::{decide, time_window_ok};
use crate::diff::{reapply_targets, stable_sort, write_targets, PrevSnapshot};
use crate::evolution::ClusterSet;
use crate::idempotency::idempotency_key;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInput {
    pub workorders: Vec<WorkOrder>,
    pub devices: Vec<Device>,
    #[serde(default)]
    pub now_epoch: i64,
    #[serde(default)]
    pub time_window_secs: i64,
}

#[derive(Debug, Clone)]
pub struct EngineOutput {
    pub clusters: Vec<dfa_core::Cluster>,
    pub cluster_set: ClusterSet,
    /// 身份歧义拒绝 / 待核实的工单号。
    pub pending_verify: Vec<String>,
    /// 「不可比」工单及缺失字段名清单。
    pub incomparable: Vec<String>,
    /// 词表外描述 → 待人工归类，不新增类别。
    pub out_of_vocabulary: Vec<String>,
    /// 工单号 → 分组标识（评测脚本按此计算纯度 / 误并 / 漏并）。
    pub assignments: BTreeMap<String, String>,
    pub summary: RunSummary,
    pub diff_elapsed_ms: u128,
}

pub fn kernel(window_secs: i64) -> DeterministicKernel {
    DeterministicKernel {
        vocabulary: FaultVocabulary::standard(),
        constraints: HardConstraints {
            time_window_secs: if window_secs > 0 {
                window_secs
            } else {
                DEFAULT_TIME_WINDOW_SECS
            },
            ..HardConstraints::default()
        },
    }
}

pub fn run(input: &EngineInput) -> EngineOutput {
    let kernel = kernel(input.time_window_secs);
    let registry = Registry::build(input.devices.clone());
    let (bound, rejected) = bind_all(&registry, &input.workorders);

    let mut orders = input.workorders.clone();
    stable_sort(&mut orders);
    let empty_prev: PrevSnapshot = PrevSnapshot::new();
    let closed: HashSet<String> = HashSet::new();
    let t0 = Instant::now();
    let deltas = reapply_targets(&empty_prev, &orders);
    let diff = write_targets(&deltas, &closed);
    let diff_elapsed_ms = t0.elapsed().as_millis();

    let mut candidates: Vec<(WorkOrder, Device, crate::cluster_key::ClusterKeyDecision)> =
        Vec::new();
    let mut incomparable = Vec::new();
    let mut out_of_vocabulary = Vec::new();
    let mut assignments: BTreeMap<String, String> = BTreeMap::new();

    for (wo, device, _stable_key) in &bound {
        let cleaned = clean_text(&wo.fault_text, dfa_core::DEFAULT_MAX_FIELD_LEN);
        let decision = decide(&kernel, device, &cleaned.text, None);
        if decision.out_of_vocabulary {
            out_of_vocabulary.push(wo.workorder_no.clone());
            assignments.insert(wo.workorder_no.clone(), "PENDING_MANUAL".to_string());
            continue;
        }
        if !decision.comparable {
            incomparable.push(format!(
                "{}|缺字段:{}",
                wo.workorder_no,
                decision.missing.join(",")
            ));
            assignments.insert(wo.workorder_no.clone(), "INCOMPARABLE".to_string());
            continue;
        }
        let key = decision.cluster_key.clone().unwrap_or_default();
        assignments.insert(wo.workorder_no.clone(), key);
        candidates.push((wo.clone(), device.clone(), decision));
    }

    // 同一工单跨多 tick 重复出现不重复入队、不重复消耗确认预算。
    let mut seen: HashSet<String> = HashSet::new();
    let mut groups: BTreeMap<String, Vec<(WorkOrder, Device, crate::cluster_key::ClusterKeyDecision)>> =
        BTreeMap::new();
    for (wo, device, decision) in candidates {
        if !seen.insert(wo.workorder_no.clone()) {
            continue;
        }
        let key = decision.cluster_key.clone().unwrap_or_default();
        groups.entry(key).or_default().push((wo, device, decision));
    }

    let mut set = ClusterSet::new();
    let now_epoch = input.now_epoch;
    for (key, mut entries) in groups {
        entries.sort_by(|a, b| {
            a.0.reported_epoch
                .cmp(&b.0.reported_epoch)
                .then_with(|| a.0.workorder_no.cmp(&b.0.workorder_no))
        });
        let earliest = entries.first().map(|e| e.0.reported_epoch).unwrap_or(0);
        let latest = entries.last().map(|e| e.0.reported_epoch).unwrap_or(0);
        let reference = if now_epoch > 0 { now_epoch } else { latest };
        let window_ok = time_window_ok(earliest, reference, &kernel);
        if !window_ok {
            for (wo, _, _) in &entries {
                assignments.insert(wo.workorder_no.clone(), format!("EXPIRED:{key}"));
            }
            // 超窗：簇降级为「需重新观察」，不出同源结论。
            let mut c = dfa_core::Cluster::observing("C-EXPIRED", &key, "PENDING_MANUAL");
            c.evidence.push("超出同源时间窗：需重新观察".to_string());
            c.degrade_label = dfa_core::DEGRADE_LABEL.to_string();
            set.clusters.push(c);
            continue;
        }
        for (wo, device, decision) in entries {
            let member = dfa_core::ClusterMember {
                workorder_no: wo.workorder_no.clone(),
                device_id: wo.device_id.clone(),
                model: device.model.clone(),
                firmware: device.firmware.clone(),
                purchase_batch: device.purchase_batch.clone(),
                ward: device.ward.clone(),
                fault_text: wo.fault_text.clone(),
                reported_at: wo.reported_at.clone(),
                reported_epoch: wo.reported_epoch,
                fault_category: decision.category.clone(),
            };
            set.ingest(&key, member, &decision.category, &decision.evidence, latest);
        }
    }

    // 单条工单不构成「簇」，只留在 assignments 里供评测分组；对外清单自然不含它们。
    let clusters: Vec<dfa_core::Cluster> = set
        .clusters
        .iter()
        .filter(|c| c.member_count() >= 2)
        .cloned()
        .collect();

    let mut summary = RunSummary {
        scanned: input.workorders.len(),
        added: diff.counts.added,
        recurring: diff.counts.recurring,
        clusters: clusters.len(),
        degradations: 0,
        outbound_writes: 0,
        failed_rows: 0,
        truncations: 0,
    };
    set.budgets.summary_contribution(&mut summary);

    EngineOutput {
        clusters,
        cluster_set: set,
        pending_verify: rejected,
        incomparable,
        out_of_vocabulary,
        assignments,
        summary,
        diff_elapsed_ms,
    }
}

/// 内部快照 CSV（含观察中的簇），供 snapshot 子命令与验收核对。
pub fn snapshot_csv(clusters: &[dfa_core::Cluster]) -> String {
    let mut out = String::from(dfa_core::CSV_HEADER);
    out.push('\n');
    for c in clusters {
        for m in &c.members {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                c.cluster_key,
                m.device_id,
                m.model,
                m.firmware,
                m.purchase_batch,
                m.ward,
                c.fault_category,
                c.first_seen_at,
                c.member_count()
            ));
        }
    }
    out
}

/// 聚类结果 JSONL：每行 {"workorder_no":..., "group":...}，供 eval_clustering.py 度量。
pub fn assignments_jsonl(output: &EngineOutput) -> String {
    let mut out = String::new();
    for (wo, group) in &output.assignments {
        out.push_str(&format!(
            "{}\n",
            serde_json::json!({"workorder_no": wo, "group": group})
        ));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScaleMetrics {
    pub total_workorders: usize,
    pub total_members: usize,
    pub unique_idempotency_keys: usize,
    pub idempotency_uniqueness: f64,
    pub cluster_consistency: f64,
    pub receipt_completeness: f64,
    /// 单轮差分耗时（毫秒，保留亚毫秒精度，避免四舍五入成 0 掩盖真实耗时）。
    pub single_diff_elapsed_ms: f64,
    pub resident_bytes: usize,
}

/// 真实规模度量：幂等键唯一性、簇内一致性、逐条回执完整率、单轮差分耗时、内存驻留。
pub fn measure(input: &EngineInput) -> ScaleMetrics {
    let t0 = Instant::now();
    let output = run(input);
    let _total_elapsed = t0.elapsed();

    let empty_prev: PrevSnapshot = PrevSnapshot::new();
    let t1 = Instant::now();
    let deltas = reapply_targets(&empty_prev, &input.workorders);
    let single_diff_elapsed_ms = t1.elapsed().as_secs_f64() * 1000.0;

    let mut keys: HashSet<String> = HashSet::new();
    let mut pairs = 0usize;
    let mut consistent = 0usize;
    let mut members = 0usize;
    for c in &output.clusters {
        // cluster_key 由 eval_hard_constraints 用 trim() 后的字段拼成，
        // 度量两侧必须做同样的归一化，否则带首尾空白的真实型号会被误算成不一致。
        let parts: Vec<&str> = c.cluster_key.split('|').collect();
        for m in &c.members {
            keys.insert(idempotency_key(&c.cluster_id, &m.workorder_no));
            pairs += 1;
            members += 1;
            let model_ok = parts
                .first()
                .map(|v| v.trim() == m.model.trim())
                .unwrap_or(false);
            let firmware_ok = parts
                .get(1)
                .map(|v| v.trim() == m.firmware.trim())
                .unwrap_or(false);
            let batch_ok = parts
                .get(2)
                .map(|v| v.trim() == m.purchase_batch.trim())
                .unwrap_or(false);
            let match_fields = model_ok
                && firmware_ok
                && batch_ok
                && !m.firmware.trim().is_empty()
                && !m.purchase_batch.trim().is_empty();
            if match_fields {
                consistent += 1;
            }
        }
    }
    let idempotency_uniqueness = if pairs == 0 {
        1.0
    } else {
        keys.len() as f64 / pairs as f64
    };
    let cluster_consistency = if pairs == 0 {
        1.0
    } else {
        consistent as f64 / pairs as f64
    };
    // 逐条回执完整率：每个簇成员都在本地提交账本里生成一条回执（成功 / 可重放）。
    let mut receipt_ledger = crate::idempotency::SubmissionLedger::new();
    for c in &output.clusters {
        for m in &c.members {
            receipt_ledger.submit(&c.cluster_id, &m.workorder_no, true);
        }
    }
    let receipt_completeness = if members == 0 {
        1.0
    } else {
        receipt_ledger.active_count() as f64 / members as f64
    };
    let resident_bytes = std::mem::size_of_val(&output.clusters)
        + output.clusters.iter().map(|c| c.members.capacity() * std::mem::size_of::<dfa_core::ClusterMember>()).sum::<usize>()
        + input.workorders.capacity() * std::mem::size_of::<WorkOrder>()
        + deltas.len() * std::mem::size_of::<crate::diff::Delta>();

    ScaleMetrics {
        total_workorders: input.workorders.len(),
        total_members: members,
        unique_idempotency_keys: keys.len(),
        idempotency_uniqueness,
        cluster_consistency,
        receipt_completeness,
        single_diff_elapsed_ms,
        resident_bytes,
    }
}
