//! `-g` on the DAG path (`--isel=dag`): the line rows and variable places of an
//! object, for clang's -O0 and -O2 fixtures. The DAG path selects other code
//! than the hand selector, so the rows are checked against the source, and the
//! variables against the hand selector's.

use std::collections::BTreeSet;
use std::sync::Mutex;

use llrm_arm64::driver::{self, IselMode, Level};
use llrm_arm64::machine::Platform;
use llrm_debug::Described;

const FIXTURES: [(&str, Level); 2] =
    [(include_str!("data/debug_o0.ll"), Level::O0), (include_str!("data/debug_o2.ll"), Level::O2)];

/// A loop with a branch in it: phis and critical edges at -O2.
const PHI_FIXTURES: [(&str, Level); 2] =
    [(include_str!("data/debug_phi_o0.ll"), Level::O0), (include_str!("data/debug_phi_o2.ll"), Level::O2)];

/// The selector mode is process-wide: one compile sets it at a time.
static MODE: Mutex<()> = Mutex::new(());

fn described(
    text: &str,
    level: Level,
    mode: IselMode,
) -> Described {
    described_on(Platform::Linux, text, level, mode)
}

fn described_on(
    platform: Platform,
    text: &str,
    level: Level,
    mode: IselMode,
) -> Described {
    let _guard = MODE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    driver::set_isel_mode(mode);
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

/// Under `--isel=dag` the line table held one row, `(0, 2, 0)`, whatever the
/// function: the source place of each IR instruction never reached the machine
/// operations its node became.
#[test]
fn the_dag_path_has_a_row_where_the_source_place_changes() {
    for (text, level) in FIXTURES {
        let hand = described(text, level, IselMode::Hand);
        let dag = described(text, level, IselMode::Dag);
        let (hand, dag) = (hand.debug.unwrap().lines, dag.debug.unwrap().lines);
        assert!(hand.len() > 2, "premise: the hand selector has rows");
        assert!(dag.len() > 2, "{level:?}: {dag:?}");
        assert!(dag.windows(2).all(|pair| pair[0].offset < pair[1].offset), "{level:?}: {dag:?}");
        assert!(dag.iter().all(|row| row.offset % 4 == 0), "{level:?}: {dag:?}");
        let dag_lines = dag.iter().map(|row| row.line).collect::<BTreeSet<_>>();
        let hand_lines = hand.iter().map(|row| row.line).collect::<BTreeSet<_>>();
        assert_eq!(dag_lines, hand_lines, "{level:?}: every source line with code has a row");
    }
}

/// The variables of the function were not named: `llvm.dbg.declare` of an
/// `alloca` never made a place.
#[test]
fn the_dag_path_places_the_variables_in_their_frame_objects() {
    let o0 = described(FIXTURES[0].0, Level::O0, IselMode::Dag);
    assert_eq!(
        variables(&o0),
        [
            ("a".to_owned(), "Frame { disp: -32 }".to_owned()),
            ("b".to_owned(), "Frame { disp: -28 }".to_owned()),
            ("t".to_owned(), "Frame { disp: -24 }".to_owned())
        ]
    );
    for (text, level) in FIXTURES {
        let names =
            |mode| variables(&described(text, level, mode)).into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        // At -O2 the DAG path also places variables the hand selector cannot
        // (`a_dag_path_at_o2_names_every_variable_the_hand_selector_does`).
        let (dag, hand) = (names(IselMode::Dag), names(IselMode::Hand));
        assert!(level != Level::O0 || dag == hand, "{level:?}");
        assert!(!dag.is_empty(), "premise: variables");
    }
}

/// The function has its body bounds, with a row at each, and a row on the line
/// before it is not a statement: what the hand selector's object has.
#[test]
fn the_dag_path_has_body_bounds_and_statement_flags() {
    for (text, level) in FIXTURES {
        let object = described(text, level, IselMode::Dag);
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
    for (text, level) in FIXTURES {
        let _guard = MODE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        driver::set_isel_mode(IselMode::Dag);
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
/// row, as the hand selector's object has.
#[test]
fn a_loop_with_phis_keeps_every_source_line_with_code() {
    for (text, level) in PHI_FIXTURES {
        let hand = row_lines(&described(text, level, IselMode::Hand));
        let dag = row_lines(&described(text, level, IselMode::Dag));
        assert!(hand.len() > 2, "premise: the hand selector has rows");
        assert_eq!(dag, hand, "{level:?}");
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

/// Mach-O: the rows and variable names are the hand selector's, as on Linux.
#[test]
fn the_dag_path_has_the_same_rows_and_variables_on_darwin() {
    for (text, level) in FIXTURES.into_iter().chain(PHI_FIXTURES) {
        let text = darwin(text);
        let hand = described_on(Platform::Darwin, &text, level, IselMode::Hand);
        let dag = described_on(Platform::Darwin, &text, level, IselMode::Dag);
        assert!(row_lines(&hand).len() > 2, "premise: the hand selector has rows");
        assert_eq!(row_lines(&dag), row_lines(&hand), "{level:?}");
        let names = |described: &Described| variables(described).into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        let (dag, hand) = (names(&dag), names(&hand));
        assert!(hand.iter().all(|name| dag.contains(name)), "{level:?}: {dag:?} against {hand:?}");
        assert!(level != Level::O0 || dag == hand, "{level:?}");
    }
}

/// A function whose `{` is on the line after its name: the first row is the
/// `scopeLine` (2), as the pin's, not the line of the name (1).
#[test]
fn the_first_row_is_the_scope_line() {
    for mode in [IselMode::Hand, IselMode::Dag] {
        let object = described(include_str!("data/debug_scope_o0.ll"), Level::O0, mode);
        let rows = &object.debug.as_ref().expect("debug information").lines;
        assert_eq!((rows[0].offset, rows[0].line), (0, 2), "{mode:?}: {rows:?}");
        assert_eq!(rows[1].line, 3, "{mode:?}: {rows:?}");
    }
}

/// A variable `llvm.dbg.value` puts in a register got no location at all at
/// -O2: only `llvm.dbg.declare` reached the machine function, so 296 of the 400
/// variables the pin places were missing in ours (dwarf_loc_diff, arm64
/// bench, 2026-11-06).
#[test]
fn a_variable_in_a_register_has_a_location_from_dbg_value() {
    // The counts are the process's: hold the selector mode, so no other compile
    // runs between the two readings.
    let _guard = MODE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    driver::set_isel_mode(IselMode::Dag);
    let before = llrm_arm64::lir_view::note_counts();
    let object = driver::object_with(include_str!("data/debug_value_o2.ll"), Platform::Linux, "t", Level::O2, true)
        .expect("lowers");
    let after = llrm_arm64::lir_view::note_counts();
    // No note may go unplaced here: a pass with most locations dropped would
    // still find some.
    let located = variables(&object);
    assert_eq!((after.0 - before.0, after.1 - before.1), (2, 2), "notes seen and placed: {located:?}");
    let place = |name: &str| located.iter().find(|(one, _)| one == name).map(|(_, place)| place.clone());
    assert!(place("n").is_some_and(|place| place.contains("Register(")), "{located:?}");
    assert!(place("m").is_some_and(|place| place.contains("Register(")), "{located:?}");
}

/// At -O2 the DAG path places the variables `llvm.dbg.value` keeps in
/// registers as well as the frame ones, so it names every variable the hand
/// selector does and may name more: a subset, not an equality. Its own test, so
/// that dropping a hand name goes red here and not only at -O0.
#[test]
fn a_dag_path_at_o2_names_every_variable_the_hand_selector_does() {
    let (text, level) = FIXTURES[1];
    let names = |mode| variables(&described(text, level, mode)).into_iter().map(|(name, _)| name).collect::<Vec<_>>();
    let (dag, hand) = (names(IselMode::Dag), names(IselMode::Hand));
    assert!(!hand.is_empty(), "premise: the hand selector names variables");
    assert!(hand.iter().all(|name| dag.contains(name)), "{dag:?} against {hand:?}");
}
