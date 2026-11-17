//! 簇状态机：观察中 → 已确认同源 → 已开单 → 已闭环。
//! 唯一推进者是人的确认动作；非法跃迁（未确认直接置已开单）被拒绝并返回原状态。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterState {
    Observing,
    ConfirmedSameSource,
    Ordered,
    Closed,
}

impl ClusterState {
    pub fn label(&self) -> &'static str {
        match self {
            ClusterState::Observing => "观察中",
            ClusterState::ConfirmedSameSource => "已确认同源",
            ClusterState::Ordered => "已开单",
            ClusterState::Closed => "已闭环",
        }
    }

    /// 合法迁移只有相邻一步，且必须由人工确认驱动。
    pub fn is_legal_next(&self, to: ClusterState) -> bool {
        matches!(
            (*self, to),
            (ClusterState::Observing, ClusterState::ConfirmedSameSource)
                | (
                    ClusterState::ConfirmedSameSource,
                    ClusterState::Ordered
                )
                | (ClusterState::Ordered, ClusterState::Closed)
        )
    }

    pub fn try_transition(
        &self,
        to: ClusterState,
        human_confirmed: bool,
    ) -> Result<ClusterState, StateError> {
        if !human_confirmed {
            return Err(StateError {
                from: *self,
                to,
                reason: "ERR_ILLEGAL_TRANSITION: 未经人工确认，状态不得推进".to_string(),
            });
        }
        if !self.is_legal_next(to) {
            return Err(StateError {
                from: *self,
                to,
                reason: "ERR_ILLEGAL_TRANSITION: 非法跃迁，返回原状态".to_string(),
            });
        }
        Ok(to)
    }

    /// 界面层用的宽容版本：失败时返回原状态，绝不静默推进。
    pub fn transition_or_self(&self, to: ClusterState, human_confirmed: bool) -> ClusterState {
        self.try_transition(to, human_confirmed).unwrap_or(*self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateError {
    pub from: ClusterState,
    pub to: ClusterState,
    pub reason: String,
}
