use dfa_config::{
    detect_renames, doctor_check, evaluate, match_key, report, resolve_strict, Config,
    MatchSource, OverlayLayer,
};
use dfa_core::{Device, PENDING_MANUAL};

const OLD: &str = "[fault_vocabulary]\nREFUSAL_TO_COOL_V0 = [\"制冷失效\"]\n";

#[test]
fn rename_is_detected_with_old_and_new_name() {
    let issues = detect_renames(OLD);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].old, "REFUSAL_TO_COOL_V0");
    assert_eq!(issues[0].new, "REFUSAL_TO_COOL");
    assert!(issues[0].message.contains("不静默沿用旧名"));
    assert!(Config::from_text(OLD).is_err());
    assert!(doctor_check(OLD).is_err());
    assert!(doctor_check("[fault_vocabulary]\nREFUSAL_TO_COOL = [\"制冷失效\"]\n").is_ok());
}

#[test]
fn missing_field_is_incomparable_with_field_names() {
    let cfg = Config::default();
    let dev = Device {
        model: "M-EXAMPLE".into(),
        serial: "SER-EXAMPLE-01".into(),
        firmware: String::new(),
        purchase_batch: String::new(),
        ..Device::default()
    };
    let r = report(&cfg, &dev, "REFUSAL_TO_COOL");
    assert!(!r.comparable);
    assert!(r.missing_fields.contains(&"firmware".to_string()));
    assert!(r.missing_fields.contains(&"purchase_batch".to_string()));
    assert!(r.note.contains("不可比"));
    assert!(!evaluate(&cfg, &dev, "REFUSAL_TO_COOL").comparable);
}

#[test]
fn config_loading_never_rewrites_the_file() {
    let dir = std::env::temp_dir().join(format!("dfa-config-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("config.toml");
    let text = "[dispatch]\ndefault_priority = \"P3\"\n";
    std::fs::write(&p, text).unwrap();
    let before = std::fs::read(&p).unwrap();
    let cfg = Config::load(&p).unwrap();
    assert_eq!(cfg.dispatch.default_priority, "P3");
    assert_eq!(std::fs::read(&p).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn overlay_uses_explicit_default_instead_of_fallback() {
    let mut cfg = Config::default();
    cfg.overrides
        .insert("default.category".into(), "REFUSAL_TO_COOL".into());
    cfg.overrides
        .insert("team.班组-A.other".into(), "NO_RESPONSE".into());
    let r = resolve_strict(&cfg, "重症医学科", "班组-A");
    assert_eq!(r.layer, OverlayLayer::Team);
    assert!(r.category.is_none());
    assert!(r.note.contains("不回填"));
}

#[test]
fn matching_is_exact_then_alias_then_pending() {
    let mut cfg = Config::default();
    cfg.fault_vocabulary
        .insert("REFUSAL_TO_COOL".into(), vec!["制冷失效".into()]);
    cfg.aliases
        .insert("没冷气".into(), "REFUSAL_TO_COOL".into());
    assert_eq!(match_key(&cfg, "REFUSAL_TO_COOL").source, MatchSource::Exact);
    assert_eq!(match_key(&cfg, "没冷气").source, MatchSource::Alias);
    let none = match_key(&cfg, "完全未知的描述");
    assert_eq!(none.source, MatchSource::None);
    assert_eq!(none.category, PENDING_MANUAL);
}

#[test]
fn with_defaults_does_not_backfill_judgement_fields() {
    let cfg = Config::default().with_defaults();
    assert_eq!(cfg.confirm_budget.new_cluster, 4);
    assert_eq!(cfg.confirm_budget.recurring_cluster, 1);
    assert!(cfg.fault_vocabulary.is_empty());
    assert!(cfg.hard_constraints().require_firmware);
}
