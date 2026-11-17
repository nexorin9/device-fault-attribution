//! T15 proof：加载 / 空 / 错误三态文案、768 折叠布局、二次确认取消后出站计数 = 0。

use dfa_desktop::{
    align_numeric, cancel_outbound, confirm_text, display_value, layout_mode, never_says_no_fault,
    polish_text, state_text, submitting_feedback, submit_feedback, DestructiveConfirm, LayoutMode,
    ViewState,
};

#[test]
fn three_view_states_have_distinct_copy_and_never_say_no_fault() {
    let loading = state_text(&ViewState::Loading, "就绪中");
    assert!(loading.contains("正在读取"));
    let empty_ok = state_text(&ViewState::Empty { count: 0 }, "就绪");
    assert_eq!(empty_ok, "确实没有新工单");
    let empty_bad = state_text(&ViewState::Empty { count: 0 }, "不可用");
    assert!(empty_bad.contains("工单源不可用"));
    let error = state_text(
        &ViewState::Error {
            code: "ERR_SOURCE_UNAVAILABLE".into(),
            text: "工单源不可用".into(),
        },
        "不可用",
    );
    assert!(error.contains("可重试"));
    for t in [&loading, &empty_ok, &empty_bad, &error] {
        assert!(never_says_no_fault(t), "不得显示「无故障」：{t}");
    }
}

#[test]
fn responsive_two_tiers_and_polish_helpers() {
    assert_eq!(layout_mode(1280), LayoutMode::TwoColumn1280);
    assert_eq!(layout_mode(768), LayoutMode::Stacked768);
    assert_eq!(layout_mode(1024), LayoutMode::TwoColumn1280);
    assert_eq!(display_value("   "), "—");
    assert_eq!(display_value("FW-1"), "FW-1");
    assert_eq!(align_numeric("12", 6), "    12");
    assert!(polish_text("不制冷", 3).starts_with("不制"));
    assert!(polish_text("一二三四五六", 3).contains("展开全文，共 6 字"));
    assert_eq!(polish_text("", 10), "—");
}

#[test]
fn destructive_confirm_restates_count_and_models_and_cancel_writes_nothing() {
    let dialog = DestructiveConfirm {
        open: true,
        order_count: 3,
        models: vec!["M-EXAMPLE".into(), "监护仪 M-220".into()],
        second_confirm: true,
    };
    let text = confirm_text(&dialog);
    assert!(text.contains("3 张工单"));
    assert!(text.contains("M-EXAMPLE"));
    assert_eq!(cancel_outbound(), 0, "取消即不产生任何出站请求");

    let submitting = submitting_feedback();
    assert!(submitting.loading && submitting.disabled_repeat_click);
    let done = submit_feedback(true, 2, 1);
    assert!(!done.loading);
    assert!(done.inline.contains("可重放"));
    let blocked = submit_feedback(false, 0, 0);
    assert!(blocked.inline.contains("出站请求计数 = 0"));
}
