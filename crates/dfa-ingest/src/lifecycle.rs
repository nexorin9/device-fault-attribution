//! 常驻生命周期状态机（源 `Booted → Wanted → Armed → Running` 转义）、
//! 轮询失败退避、休眠 / 唤醒 I/O 门控与「无人使用即待命」。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleState {
    Booted,
    Wanted,
    Armed,
    Running,
    Dormant,
}

impl LifecycleState {
    pub fn label(&self) -> &'static str {
        match self {
            LifecycleState::Booted => "启动",
            LifecycleState::Wanted => "待命",
            LifecycleState::Armed => "就绪待命",
            LifecycleState::Running => "在役",
            LifecycleState::Dormant => "休眠",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lifecycle {
    pub state: LifecycleState,
    pub base_poll_ms: u64,
    pub current_poll_ms: u64,
    pub consecutive_failures: u32,
    pub wake_refresh_count: usize,
    pub outbound_during_sleep: usize,
    pub backoff_events: usize,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Lifecycle {
            state: LifecycleState::Booted,
            base_poll_ms: 60_000,
            current_poll_ms: 60_000,
            consecutive_failures: 0,
            wake_refresh_count: 0,
            outbound_during_sleep: 0,
            backoff_events: 0,
        }
    }
}

impl Lifecycle {
    pub fn new() -> Self {
        Lifecycle::default()
    }

    pub fn is_legal_next(&self, to: LifecycleState) -> bool {
        matches!(
            (self.state, to),
            (LifecycleState::Booted, LifecycleState::Wanted)
                | (LifecycleState::Wanted, LifecycleState::Armed)
                | (LifecycleState::Armed, LifecycleState::Running)
                | (LifecycleState::Running, LifecycleState::Wanted)
                | (LifecycleState::Wanted, LifecycleState::Running)
                | (LifecycleState::Running, LifecycleState::Dormant)
                | (LifecycleState::Dormant, LifecycleState::Running)
        )
    }

    pub fn transition(&mut self, to: LifecycleState) -> Result<LifecycleState, String> {
        if !self.is_legal_next(to) {
            return Err(format!(
                "非法跃迁 {} → {}，返回原状态",
                self.state.label(),
                to.label()
            ));
        }
        self.state = to;
        Ok(to)
    }

    /// 连续失败按指数退避放宽轮询周期；恢复后回到基准周期。
    pub fn poll_failure(&mut self) -> u64 {
        self.consecutive_failures += 1;
        self.backoff_events += 1;
        let factor = 1u64 << self.consecutive_failures.min(4);
        self.current_poll_ms = self.base_poll_ms.saturating_mul(factor);
        self.current_poll_ms
    }

    pub fn poll_success(&mut self) {
        self.consecutive_failures = 0;
        self.current_poll_ms = self.base_poll_ms;
    }

    /// 休眠：停止轮询与出站 I/O。
    pub fn sleep(&mut self) {
        self.state = LifecycleState::Dormant;
        self.outbound_during_sleep = 0;
    }

    /// 唤醒：强制刷新恰好一次，再回到常规周期。
    pub fn wake(&mut self) {
        if self.state == LifecycleState::Dormant {
            self.state = LifecycleState::Running;
            self.wake_refresh_count += 1;
            self.poll_success();
        }
    }

    /// 出站 I/O 门控：休眠期一律不发起请求。
    pub fn allow_outbound(&self) -> bool {
        self.state != LifecycleState::Dormant
    }

    /// 一段时间无窗口交互则进入待命（保留托盘与轮询）。
    pub fn mark_idle(&mut self) {
        if self.state == LifecycleState::Running {
            self.state = LifecycleState::Wanted;
        }
    }

    pub fn interact(&mut self) {
        if self.state == LifecycleState::Wanted {
            self.state = LifecycleState::Running;
        }
    }
}
