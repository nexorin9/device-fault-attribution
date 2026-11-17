//! 规则守门、草稿边界与人工确认落库。
//! 模型产出全部草稿；只有人的确认动作把簇推进到「已确认同源」并产生写操作。
//! 确认记录含操作人、时间与成员快照，使「确认时簇里有哪些设备」可回查。

use dfa_core::{ClusterState, FaultVocabulary, TeamCapability};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmRequest {
    pub category: String,
    pub members_verified: bool,
    pub team: String,
    pub capabilities: Vec<TeamCapability>,
    pub earliest_epoch: i64,
    pub now_epoch: i64,
    pub window_secs: i64,
    pub actor: String,
    pub ts: i64,
    pub member_snapshot: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Confirmation {
    pub actor: String,
    pub ts: i64,
    pub member_snapshot: Vec<String>,
    pub before_state: String,
    pub after_state: String,
    pub items: Vec<String>,
    pub draft: bool,
}

pub fn time_window_expired(req: &ConfirmRequest) -> bool {
    if req.window_secs <= 0 || req.now_epoch <= 0 {
        return false;
    }
    req.now_epoch.saturating_sub(req.earliest_epoch) > req.window_secs
}

/// 确认前置校验：故障类别受控、成员全部已核实、建议班组在能力表内；
/// 三者任一不满足（或时间窗过期）即拒绝确认并列出缺项。
pub fn confirm(
    req: &ConfirmRequest,
    state: ClusterState,
) -> Result<Confirmation, Vec<String>> {
    let mut missing = Vec::new();
    let vocab = FaultVocabulary::standard();
    if !vocab.contains(&req.category) {
        missing.push(format!("故障类别 {} 不在受控词表内", req.category));
    }
    if !req.members_verified {
        missing.push("簇内存在未核实设备".to_string());
    }
    if !req.capabilities.iter().any(|t| t.team == req.team) {
        missing.push(format!("建议班组 {} 不在当次重取的能力表内", req.team));
    }
    if time_window_expired(req) {
        missing.push("簇内最早故障时间超出同源时间窗：需重新观察，不出同源结论、不生成工单".to_string());
    }
    if !missing.is_empty() {
        return Err(missing);
    }
    let after = state
        .try_transition(ClusterState::ConfirmedSameSource, true)
        .map_err(|e| vec![e.reason])?;
    Ok(Confirmation {
        actor: req.actor.clone(),
        ts: req.ts,
        member_snapshot: req.member_snapshot.clone(),
        before_state: state.label().to_string(),
        after_state: after.label().to_string(),
        items: vec![
            format!("类别 {}", req.category),
            format!("班组 {}", req.team),
        ],
        draft: false,
    })
}
