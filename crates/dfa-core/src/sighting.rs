//! 三值在场性：读不到 ≠ 不在。Unknown（读失败 / 字段缺失）打断缺席计数，
//! 而不是延长它；连续 Absent 达到阈值才判「不在」。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sighting {
    Seen,
    Absent,
    Unknown,
}

impl Sighting {
    pub fn label(&self) -> &'static str {
        match self {
            Sighting::Seen => "在场",
            Sighting::Absent => "缺席",
            Sighting::Unknown => "未知（读不到）",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsenceCounter {
    pub threshold: u32,
    pub consecutive_absent: u32,
    pub gone: bool,
    pub unknown_breaks: u32,
}

impl AbsenceCounter {
    pub fn new(threshold: u32) -> Self {
        AbsenceCounter {
            threshold: threshold.max(1),
            consecutive_absent: 0,
            gone: false,
            unknown_breaks: 0,
        }
    }

    pub fn observe(&mut self, sighting: Sighting) {
        match sighting {
            Sighting::Seen => {
                self.consecutive_absent = 0;
                self.gone = false;
            }
            Sighting::Absent => {
                self.consecutive_absent += 1;
                if self.consecutive_absent >= self.threshold {
                    self.gone = true;
                }
            }
            Sighting::Unknown => {
                // 读不到打断缺席计数：既不延长缺席，也不把未知当成「已离场」。
                self.consecutive_absent = 0;
                self.unknown_breaks += 1;
            }
        }
    }

    pub fn is_gone(&self) -> bool {
        self.gone
    }
}
