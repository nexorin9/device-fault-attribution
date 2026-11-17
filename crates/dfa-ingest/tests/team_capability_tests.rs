//! T36 proof：能力源不可用时建单被拒且出站写请求计数 = 0、
//! 两次建单之间能力源请求计数递增（不缓存）。

use dfa_core::TeamCapability;
use dfa_ingest::{Readiness, SourceError, TeamCapabilitySource};

fn teams() -> Vec<TeamCapability> {
    vec![
        TeamCapability {
            team: "设备维修一组".into(),
            capabilities: vec!["检查制冷回路".into(), "核查固件版本".into()],
        },
        TeamCapability {
            team: "厂商支持组".into(),
            capabilities: vec!["刷固件".into()],
        },
    ]
}

#[test]
fn each_order_creation_refetches_capabilities() {
    let src = TeamCapabilitySource::new(teams());
    assert_eq!(src.request_count.get(), 0);
    let first = src.fetch("检查制冷回路").unwrap();
    assert_eq!(src.request_count.get(), 1);
    assert_eq!(first.len(), 1);
    let second = src.fetch("刷固件").unwrap();
    assert_eq!(src.request_count.get(), 2, "绝不缓存过期能力");
    assert_eq!(second[0].team, "厂商支持组");
}

#[test]
fn unavailable_capability_source_forbids_order_and_writes_nothing() {
    let mut src = TeamCapabilitySource::new(teams());
    src.available = false;
    let err = src.fetch("检查制冷回路").unwrap_err();
    assert!(matches!(err, SourceError::Unavailable(_)));
    assert!(format!("{err:?}").contains("禁止建单"));
    assert_eq!(src.readiness(), Readiness::Unavailable);
    assert_eq!(src.outbound_writes.get(), 0, "出站写请求计数 = 0");
    // 没有命中能力表时同样不得建单。
    let empty = TeamCapabilitySource::new(Vec::new());
    assert!(empty.fetch("检查制冷回路").unwrap().is_empty());
}

#[test]
fn capability_set_feeds_step_ordering_and_removal() {
    let src = TeamCapabilitySource::new(teams());
    let available = src.fetch("检查制冷回路").unwrap();
    assert!(available.iter().any(|t| t.can("检查制冷回路")));
    assert!(!available.iter().any(|t| t.can("返厂换主板")));
}
