use super::compare;

const OLD: &str = "A f 0 NMUP\nC f 3 0RWB\n";

/// A pair the new stack proves disjoint that the old one did not would let a
/// pass reorder two accesses that may touch one byte: the gate fails it.
#[test]
fn a_pair_that_becomes_no_alias_is_gained() {
    let report = compare(OLD, "A f 0 NNUP\nC f 3 0RWB\n");
    assert_eq!(report.gained, ["A f 0 [1]: M -> N"]);
    assert!(!report.passes());
}

#[test]
fn a_call_that_stops_touching_a_location_is_gained() {
    let report = compare(OLD, "A f 0 NMUP\nC f 3 00WB\n");
    assert_eq!(report.gained, ["C f 3 [1]: R -> 0"]);
}

/// Losing a proof is precision, counted and not a failure.
#[test]
fn a_lost_proof_is_counted_not_failed() {
    let report = compare(OLD, "A f 0 MMUP\nC f 3 BRWB\n");
    assert!(report.passes(), "{report:?}");
    assert_eq!(report.lost, 2);
    assert_eq!(report.changed, 0);
}

#[test]
fn a_row_in_one_recording_only_fails() {
    assert!(!compare(OLD, "A f 0 NMUP\n").passes());
    assert!(!compare("A f 0 NMUP\n", OLD).passes());
}

#[test]
fn identical_recordings_pass() {
    let report = compare(OLD, OLD);
    assert!(report.passes());
    assert_eq!(report.same, 8);
}

/// A weaker answer that is no proof either way (must to partial, read to
/// write) is a change, neither lost nor gained.
#[test]
fn an_answer_with_no_proof_either_way_is_a_change() {
    let report = compare(OLD, "A f 0 NMPU\nC f 3 0WRB\n");
    assert!(report.passes(), "{report:?}");
    assert_eq!((report.lost, report.changed), (0, 4));
}
