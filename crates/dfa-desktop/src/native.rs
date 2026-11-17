//! 原生窗口层（`--features native`）：egui/eframe 单窗口两栏。
//! 默认构建不编译本模块 —— 自动化验证环境无显示服务，窗口逻辑由
//! `window::SnapshotView` 的纯数据投影承载，测试可在无 GUI 的环境跑通。

use crate::tokens;
use crate::window::SnapshotView;

/// 两栏绘制：左 = 设备注册表（含待核实分区），右 = 簇区。
pub fn draw(ctx: &egui::Context, view: &SnapshotView) {
    let bg = tokens::SURFACE_RGB;
    let fg = tokens::TEXT_RGB;
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = egui::Color32::from_rgb(bg.0, bg.1, bg.2);
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(fg.0, fg.1, fg.2));
    ctx.set_style(style);

    egui::TopBottomPanel::top("banner").show(ctx, |ui| {
        ui.label(&view.banner);
    });
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.columns(2, |cols| {
            cols[0].heading("设备注册表");
            for r in &view.registry_rows {
                cols[0].label(format!(
                    "{} | {} | {} | {} | {}",
                    r.device_id, r.model, r.firmware, r.purchase_batch, r.ward
                ));
            }
            cols[0].separator();
            cols[0].label(format!("待核实：{} 台", view.pending_rows.len()));
            for p in &view.pending_rows {
                cols[0].label(format!("{} | 缺 {}", p.device_id, p.missing.join("/")));
            }
            cols[1].heading("故障簇");
            for c in &view.cluster_cards {
                cols[1].group(|ui| {
                    ui.label(format!(
                        "{} · {} · {} 台设备",
                        c.cluster_id,
                        c.state,
                        c.member_rows.len()
                    ));
                    for m in &c.member_rows {
                        ui.label(format!(
                            "{} | {} | {} | {}",
                            m.device_id, m.model, m.firmware, m.purchase_batch
                        ));
                    }
                    ui.label(&c.draft_label);
                    for (i, s) in c.steps.iter().enumerate() {
                        ui.label(format!("{}. {s}", i + 1));
                    }
                });
            }
        });
    });
}

pub struct DfaApp {
    pub view: SnapshotView,
}

impl eframe::App for DfaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        draw(ctx, &self.view);
    }
}
