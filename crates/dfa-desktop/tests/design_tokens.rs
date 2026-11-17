//! T13 proof：设计 token 语义色映射、四态渲染、8px 栅格、字体层级，
//! 且除 tokens.rs 外源码中不出现裸色值。

use dfa_core::ClusterState;
use dfa_desktop::{
    basic_widgets, semantic_color, semantic_note, spacing, unavailable_color, FONT_SCALE, GRID,
    FOUR_STATES, Widget, WidgetState,
};
use std::path::PathBuf;

fn has_bare_hex(text: &str) -> bool {
    let b = text.as_bytes();
    for i in 0..b.len().saturating_sub(6) {
        if b[i] == b'#' && b[i + 1..i + 7].iter().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }
    }
    false
}

#[test]
fn semantic_colors_and_notes_are_distinct_for_every_cluster_state() {
    let states = [
        ClusterState::Observing,
        ClusterState::ConfirmedSameSource,
        ClusterState::Ordered,
        ClusterState::Closed,
    ];
    let mut colors = Vec::new();
    for s in states {
        let color = semantic_color(s);
        assert!(color.starts_with('#'));
        assert_eq!(color.len(), 7);
        assert!(!semantic_note(s).is_empty(), "颜色不能单独承载信息");
        colors.push(color);
    }
    assert_eq!(colors[0], dfa_desktop::OBSERVING);
    assert_eq!(colors[1], dfa_desktop::CONFIRMED);
    assert_eq!(colors[2], dfa_desktop::ORDERED);
    assert!(unavailable_color().starts_with('#'));
    assert!(dfa_desktop::pending_color().starts_with('#'));
    assert!(dfa_desktop::error_color().starts_with('#'));
}

#[test]
fn every_basic_widget_renders_all_four_states() {
    let widgets = basic_widgets();
    assert_eq!(widgets.len(), 5);
    assert_eq!(FOUR_STATES.len(), 4);
    for w in &widgets {
        let renders = w.render_four_states();
        assert_eq!(renders.len(), 4);
        for r in &renders {
            assert!(!r.color_token.is_empty());
            assert!(!r.note.is_empty(), "每个状态都要有说明文案");
        }
        let disabled = w.render(WidgetState::Disabled);
        assert!(!disabled.interactive);
        assert!(disabled.note.contains("禁用"));
        let loading = w.render(WidgetState::Loading);
        assert!(!loading.interactive);
        assert!(loading.note.contains("处理中"));
        assert!(w.render(WidgetState::Hover).interactive);
        assert!(w.render(WidgetState::Active).interactive);
    }
    let badge = Widget::status_badge("已确认同源", dfa_desktop::CONFIRMED);
    assert_eq!(badge.render(WidgetState::Normal).color_token, dfa_desktop::CONFIRMED);
}

#[test]
fn eight_pixel_grid_and_font_hierarchy_are_defined() {
    assert_eq!(GRID, 8);
    assert_eq!(spacing(1), 8);
    assert_eq!(spacing(3), 24);
    assert!(FONT_SCALE.title_px > FONT_SCALE.body_px);
    assert!(FONT_SCALE.body_px > FONT_SCALE.caption_px);
    assert!(FONT_SCALE.title_line > FONT_SCALE.title_px);
    assert!(FONT_SCALE.body_line > FONT_SCALE.body_px);
    assert!(FONT_SCALE.numeric_line > FONT_SCALE.numeric_px);
}

#[test]
fn tokens_rs_is_the_only_source_file_with_bare_hex_values() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0usize;
    for entry in std::fs::read_dir(&src).unwrap().flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        if p.file_name().and_then(|n| n.to_str()) == Some("tokens.rs") {
            assert!(has_bare_hex(&text), "色板应集中在 tokens.rs");
            continue;
        }
        assert!(
            !has_bare_hex(&text),
            "源码 {} 出现裸色值，请改用设计 token",
            p.display()
        );
        checked += 1;
    }
    assert!(checked >= 4, "除 tokens.rs 外至少应检查 4 个源码文件");
}
