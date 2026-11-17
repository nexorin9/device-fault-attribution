//! T44 proof：托盘计数与窗口待处理数一致、托盘不可用时走降级路径。

use dfa_core::{Cluster, ClusterMember};
use dfa_desktop::{SnapshotView, TrayState};

fn view(n: usize) -> SnapshotView {
    let mut clusters = Vec::new();
    for i in 0..n {
        let mut c = Cluster::observing(&format!("C-{i}"), "K", "REFUSAL_TO_COOL");
        c.members.push(ClusterMember {
            device_id: format!("DEV-{i}"),
            ..ClusterMember::default()
        });
        clusters.push(c);
    }
    SnapshotView::from_clusters(&clusters, "就绪")
}

#[test]
fn tray_count_matches_window_pending_count() {
    let v = view(3);
    let tray = TrayState::from_view(&v);
    assert_eq!(tray.pending_count, v.pending_count());
    assert_eq!(tray.new_cluster_count, 3);
    assert!(tray.matches_window(&v));
    assert_eq!(tray.title(), "设备归因 · 3 条待处理");
    let lines = tray.menu_lines();
    assert!(lines[0].contains("待处理条数：3"));
    assert!(lines[1].contains("新增簇数：3"));
    assert!(lines.iter().any(|l| l == "打开窗口"));
    assert!(lines.iter().any(|l| l == "退出"));
    assert!(tray.click_action().contains("唤起窗口"));
}

#[test]
fn unavailable_tray_degrades_to_window_badge_and_in_app_banner() {
    let v = view(2);
    let mut tray = TrayState::from_view(&v);
    assert!(tray.degraded_banner().is_none());
    tray.available = false;
    let banner = tray.degraded_banner().unwrap();
    assert!(banner.contains("托盘不可用"));
    assert!(banner.contains("应用内横幅"));
    assert!(banner.contains("2 条待处理"), "降级不丢计数");
    assert!(tray.matches_window(&v), "降级不影响主路径一致性");
    assert_eq!(TrayState::default().title(), "设备归因 · 无待处理");
}
