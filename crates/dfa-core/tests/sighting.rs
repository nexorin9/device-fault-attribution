use dfa_core::{AbsenceCounter, Sighting};

#[test]
fn three_values_have_distinct_labels() {
    assert_eq!(Sighting::Seen.label(), "在场");
    assert_eq!(Sighting::Absent.label(), "缺席");
    assert!(Sighting::Unknown.label().contains("读不到"));
}

#[test]
fn unknown_breaks_absence_count_instead_of_extending_it() {
    let mut c = AbsenceCounter::new(3);
    c.observe(Sighting::Absent);
    c.observe(Sighting::Absent);
    assert_eq!(c.consecutive_absent, 2);
    c.observe(Sighting::Unknown);
    assert_eq!(c.consecutive_absent, 0);
    assert_eq!(c.unknown_breaks, 1);
    assert!(!c.is_gone());
    c.observe(Sighting::Absent);
    assert_eq!(c.consecutive_absent, 1);
    assert!(!c.is_gone());
}

#[test]
fn only_consecutive_absent_reaches_threshold() {
    let mut c = AbsenceCounter::new(3);
    for _ in 0..3 {
        c.observe(Sighting::Absent);
    }
    assert!(c.is_gone());
    c.observe(Sighting::Seen);
    assert!(!c.is_gone());
    assert_eq!(c.consecutive_absent, 0);
}
