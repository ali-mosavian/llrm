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

/// The selector mode is process-wide: one compile sets it at a time.
static MODE: Mutex<()> = Mutex::new(());

fn described(
    text: &str,
    level: Level,
    mode: IselMode,
) -> Described {
    let _guard = MODE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    driver::set_isel_mode(mode);
    driver::object_with(text, Platform::Linux, "t", level, true).expect("lowers")
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
        assert_eq!(names(IselMode::Dag), names(IselMode::Hand), "{level:?}");
        assert!(!names(IselMode::Dag).is_empty(), "premise: variables");
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
