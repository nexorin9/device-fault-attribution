//! 桌面窗口入口（仅在 `--features native` 下编译）。
//! macOS 托盘图标须在主线程、且事件循环运行前创建；本轮可达形态为
//! 托盘计数 + 窗口内横幅 + 点击托盘唤起窗口，不画不存在的 OS toast 气泡。

fn main() -> eframe::Result<()> {
    let view = std::env::var("DFA_SNAPSHOT_FILE")
        .ok()
        .and_then(|p| {
            dfa_desktop::SnapshotView::from_snapshot_file(std::path::Path::new(&p)).ok()
        })
        .unwrap_or_default();

    // 托盘不可用时退化为「窗口置顶徽标 + 应用内横幅」，不牺牲主路径。
    let tray = dfa_desktop::TrayState::from_view(&view);
    if let Some(banner) = tray.degraded_banner() {
        eprintln!("{banner}");
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "医疗设备故障归因系统",
        options,
        Box::new(
            |_cc| -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>> {
                Ok(Box::new(dfa_desktop::native::DfaApp { view }))
            },
        ),
    )
}
