use dfa_store::{read_snapshot, simulate_interrupted_write, write_snapshot, SCHEMA_VERSION};
use tempfile::tempdir;

#[test]
fn write_then_read_roundtrip() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("nested/snapshot.json");
    write_snapshot(&p, "clusters", "PAYLOAD-COMPLETE", 1741000000).unwrap();
    let r = read_snapshot(&p).unwrap();
    assert_eq!(r.envelope.payload, "PAYLOAD-COMPLETE");
    assert_eq!(r.envelope.name, "clusters");
    assert_eq!(r.envelope.schema_version, SCHEMA_VERSION);
    assert!(r.degraded_note.is_none());
}

#[test]
fn interrupted_write_leaves_previous_snapshot_intact() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("snapshot.json");
    write_snapshot(&p, "clusters", "OLD-COMPLETE", 1).unwrap();
    simulate_interrupted_write(&p, "{\"half\":").unwrap();
    let r = read_snapshot(&p).unwrap();
    assert_eq!(r.envelope.payload, "OLD-COMPLETE");
}

#[test]
fn old_version_snapshot_reads_with_degraded_note() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("old.json");
    let old = "{\"schema_version\":1,\"name\":\"clusters\",\"payload\":\"OLD\",\"written_at\":1}";
    std::fs::write(&p, old).unwrap();
    let r = read_snapshot(&p).unwrap();
    assert_eq!(r.envelope.payload, "OLD");
    assert!(r.degraded_note.unwrap().contains("低于当前"));
}
