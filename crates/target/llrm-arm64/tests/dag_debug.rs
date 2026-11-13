//! `-g` on the DAG path: the line rows and variable places of an object, for
//! clang's -O0 and -O2 fixtures. The expected source lines and variable names
//! are those of the deleted hand selector's objects, recorded 2026-11-09.

use std::collections::BTreeSet;
use std::sync::Mutex;

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;
use llrm_debug::Described;

const FIXTURES: [(&str, Level); 2] =
    [(include_str!("data/debug_o0.ll"), Level::O0), (include_str!("data/debug_o2.ll"), Level::O2)];

/// A loop with a branch in it: phis and critical edges at -O2.
const PHI_FIXTURES: [(&str, Level); 2] =
    [(include_str!("data/debug_phi_o0.ll"), Level::O0), (include_str!("data/debug_phi_o2.ll"), Level::O2)];

/// `note_counts` is process-wide: one compile runs at a time.
static COMPILE: Mutex<()> = Mutex::new(());

/// The source lines with code in each fixture.
const FIXTURE_LINES: [&[u32]; 2] = [&[2, 3, 4], &[6, 11, 12, 13, 14, 15, 16, 17, 18, 20, 25, 26, 27]];
const PHI_LINES: [&[u32]; 2] = [&[1, 2, 3, 4, 5, 7, 9], &[1, 3, 4, 9]];

fn lines_of(expected: &[u32]) -> BTreeSet<u32> {
    expected.iter().copied().collect()
}

fn described(
    text: &str,
    level: Level,
) -> Described {
    described_on(Platform::Linux, text, level)
}

fn described_on(
    platform: Platform,
    text: &str,
    level: Level,
) -> Described {
    let _guard = COMPILE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    driver::object_with(text, platform, "t", level, true).expect("lowers")
}

fn variables(described: &Described) -> Vec<(String, String)> {
    let info = described.debug.as_ref().expect("debug information");
    info.functions
        .iter()
        .flat_map(|one| {
            one.variables.iter().map(|variable| (variable.name.clone(), format!("{:?}", variable.location)))
        })
        .collect()
}

/// The line table held one row, `(0, 2, 0)`, whatever the function: the source
/// place of each IR instruction never reached the machine operations its node
/// became.
#[test]
fn the_dag_path_has_a_row_where_the_source_place_changes() {
    for ((text, level), want) in FIXTURES.into_iter().zip(FIXTURE_LINES) {
        let dag = described(text, level).debug.unwrap().lines;
        assert!(dag.len() > 2, "{level:?}: {dag:?}");
        assert!(dag.windows(2).all(|pair| pair[0].offset < pair[1].offset), "{level:?}: {dag:?}");
        assert!(dag.iter().all(|row| row.offset % 4 == 0), "{level:?}: {dag:?}");
        let dag_lines = dag.iter().map(|row| row.line).collect::<BTreeSet<_>>();
        assert_eq!(dag_lines, lines_of(want), "{level:?}: every source line with code has a row");
    }
}

/// The variables of the function were not named: `llvm.dbg.declare` of an
/// `alloca` never made a place.
#[test]
fn the_dag_path_places_the_variables_in_their_frame_objects() {
    let o0 = described(FIXTURES[0].0, Level::O0);
    assert_eq!(
        variables(&o0),
        [
            ("a".to_owned(), "Frame { disp: -16 }".to_owned()),
            ("b".to_owned(), "Frame { disp: -12 }".to_owned()),
            ("t".to_owned(), "Frame { disp: -8 }".to_owned())
        ]
    );
}

/// The function has its body bounds, with a row at each, and a row on the line
/// before it is not a statement: as the pin's.
#[test]
fn the_dag_path_has_body_bounds_and_statement_flags() {
    for (text, level) in FIXTURES {
        let object = described(text, level);
        let info = object.debug.as_ref().expect("debug information");
        let function = info.functions.first().expect("a function");
        let (start, end) = function.body.expect("the body's bounds");
        let range = &function.ranges[0];
        assert!(0 < start && start < end && end < range.length, "{start}..{end} of {}", range.length);
        assert!(
            info.lines.iter().any(|one| one.offset == range.offset + start)
                && info.lines.iter().any(|one| one.offset == range.offset + end),
            "{level:?}: a row at each bound: {:?}",
            info.lines
        );
        let mut previous = None;
        let mut expected = BTreeSet::new();
        for row in &info.lines {
            if previous == Some(row.line) {
                expected.insert((0, row.offset));
            }
            previous = Some(row.line);
        }
        assert_eq!(info.not_stmt, expected, "{level:?}");
    }
}

/// `-g` must not change the code the DAG path makes.
#[test]
fn debug_information_leaves_the_dag_code_as_it_was() {
    let _guard = COMPILE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    for (text, level) in FIXTURES {
        let code = |debug| {
            driver::object_with(text, Platform::Linux, "t", level, debug).expect("lowers").object.sections[0]
                .image
                .clone()
        };
        assert_eq!(code(false), code(true), "{level:?}");
    }
}

fn row_lines(described: &Described) -> BTreeSet<u32> {
    described.debug.as_ref().expect("debug information").lines.iter().map(|row| row.line).collect()
}

/// The copies of a phi and the blocks of a critical edge are made by the DAG
/// path after the lines are known: a source line of the loop must still have a
/// row.
#[test]
fn a_loop_with_phis_keeps_every_source_line_with_code() {
    for ((text, level), want) in PHI_FIXTURES.into_iter().zip(PHI_LINES) {
        assert_eq!(row_lines(&described(text, level)), lines_of(want), "{level:?}");
    }
}

/// The fixture for Mach-O: clang's Darwin datalayout and triple.
fn darwin(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with("target datalayout") {
                "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\""
                    .to_owned()
            } else if line.starts_with("target triple") {
                "target triple = \"arm64-apple-macosx11.0.0\"".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Mach-O: the rows and variable names are those of Linux.
#[test]
fn the_dag_path_has_the_same_rows_and_variables_on_darwin() {
    for (text, level) in FIXTURES.into_iter().chain(PHI_FIXTURES) {
        let linux = described(text, level);
        let dag = described_on(Platform::Darwin, &darwin(text), level);
        assert_eq!(row_lines(&dag), row_lines(&linux), "{level:?}");
        let names = |described: &Described| variables(described).into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        assert_eq!(names(&dag), names(&linux), "{level:?}");
    }
}

/// A function whose `{` is on the line after its name: the first row is the
/// `scopeLine` (2), as the pin's, not the line of the name (1).
#[test]
fn the_first_row_is_the_scope_line() {
    let object = described(include_str!("data/debug_scope_o0.ll"), Level::O0);
    let rows = &object.debug.as_ref().expect("debug information").lines;
    assert_eq!((rows[0].offset, rows[0].line), (0, 2), "{rows:?}");
    assert_eq!(rows[1].line, 3, "{rows:?}");
}

/// A variable `llvm.dbg.value` puts in a register got no location at all at
/// -O2: only `llvm.dbg.declare` reached the machine function, so 296 of the 400
/// variables the pin places were missing in ours (dwarf_loc_diff, arm64
/// bench, 2026-11-06).
#[test]
fn a_variable_in_a_register_has_a_location_from_dbg_value() {
    // The counts are the process's: no other compile may run between the two
    // readings.
    let _guard = COMPILE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = llrm_arm64::lir_view::note_counts();
    let object = driver::object_with(include_str!("data/debug_value_o2.ll"), Platform::Linux, "t", Level::O2, true)
        .expect("lowers");
    let after = llrm_arm64::lir_view::note_counts();
    // No note may go unplaced here: a pass with most locations dropped would
    // still find some.
    let located = variables(&object);
    // Five notes: `n`, `m`, `k` twice (the second one `poison`) and `x`, a
    // variable of another function (an inlined callee's). `k` ends up with
    // none: a note that says nothing takes the variable's earlier note with it,
    // or its register would be named over code that no longer holds it. `x` is
    // placed but not listed: it is not this function's.
    assert_eq!((after.0 - before.0, after.1 - before.1), (5, 3), "notes seen and placed: {located:?}");
    assert!(located.iter().all(|(name, _)| name != "k" && name != "x"), "{located:?}");
    let place = |name: &str| located.iter().find(|(one, _)| one == name).map(|(_, place)| place.clone());
    assert!(place("n").is_some_and(|place| place.contains("Register(")), "{located:?}");
    assert!(place("m").is_some_and(|place| place.contains("Register(")), "{located:?}");
}

/// At -O2 the DAG path places the variables `llvm.dbg.value` keeps in
/// registers as well as the frame ones. The hand selector named only the first.
#[test]
fn a_dag_path_at_o2_names_the_variable_dbg_value_places() {
    let (text, level) = FIXTURES[1];
    let names = variables(&described(text, level)).into_iter().map(|(name, _)| name).collect::<Vec<_>>();
    assert_eq!(names, ["composite"], "{names:?}");
}
