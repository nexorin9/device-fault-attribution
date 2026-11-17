//! T16 proof（命令层）：doctor 三列表格、replay 零出站、snapshot 只读、
//! 运行摘要与 8 个环境变量键位。

use dfa_cli::{
    doctor_report, replay_report, summary_line, tray_report, view_report, ENV_KEYS,
};
use dfa_core::RunSummary;

#[test]
fn doctor_emits_the_three_column_checklist() {
    let (text, _code) = doctor_report();
    assert!(text.starts_with("检查项 | 结论 | 建议动作"));
    assert!(text.contains("--- | --- | ---"));
    assert!(text.contains("config.toml 可解析"));
    assert!(text.contains("词表改名迁移"));
    assert!(text.contains("工单源连通性"));
    for key in ENV_KEYS {
        assert!(text.contains(key), "doctor 必须逐项列出 {key}");
    }
    assert!(!text.contains("WORKORDER_TOKEN=SECRET"), "绝不打印密钥值");
}

#[test]
fn replay_and_view_are_read_only_with_zero_outbound() {
    let replay = replay_report("C-EXAMPLE-1");
    assert!(replay.contains("结果一致 = true"));
    assert!(replay.contains("出站请求计数 = 0"));
    assert!(replay.contains("replay 不写库"));
    let view = view_report();
    assert!(view.contains("出站请求 0"));
    assert!(view.contains("左栏") && view.contains("右栏"));
}

#[test]
fn tray_report_carries_pending_count_and_menu() {
    let text = tray_report();
    assert!(text.contains("托盘项标题"));
    assert!(text.contains("待处理条数"));
    assert!(text.contains("新增簇数"));
    assert!(text.contains("打开窗口"));
}

#[test]
fn summary_line_contains_all_required_counters() {
    let s = RunSummary {
        scanned: 120,
        added: 7,
        recurring: 4,
        clusters: 3,
        degradations: 2,
        outbound_writes: 0,
        failed_rows: 1,
        truncations: 1,
    };
    let line = summary_line(&s);
    for key in ["扫描", "新增", "复现", "成簇", "降级", "出站写", "脏行", "截断"] {
        assert!(line.contains(key), "摘要缺 {key}");
    }
    assert!(line.contains("扫描=120"));
    assert_eq!(ENV_KEYS.len(), 8);
}
