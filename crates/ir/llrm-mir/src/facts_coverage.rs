//! Every fact is tested from the language to a pass: its carrier made from
//! what a frontend states, its accessor, and an optimisation that reads it.
//! A fact gets a row here when it lands, naming the tests that show it, or
//! says whose reader is not built yet; a fact with neither fails the build.

use crate::facts::Fact;

/// What shows a fact end to end.
enum Shown {
    /// Tests by name, each a `fn` somewhere in the workspace.
    By(&'static [&'static str]),
    /// Stated and carried, no pass reads it yet: whose reader, and why it waits.
    Unread(&'static str),
}

use Shown::{By, Unread};

const COVERAGE: &[(&str, Shown)] = &[
    ("noalias", By(&["a_stated_fact_becomes_its_carrier", "test_restrict_parameters_are_noalias", "noalias_parameters_are_disjoint_and_plain_ones_may_alias"])),
    ("readonly", By(&["test_a_load_from_a_readonly_noalias_parameter_leaves_past_a_store"])),
    ("readnone", By(&["a_readnone_or_memory_none_callee_touches_nothing"])),
    ("nonnull", By(&["a_frame_object_is_nonnull_and_a_parameter_is_not"])),
    ("nocapture", By(&["facts_of_a_call_argument_are_its_call_site_attributes"])),
    ("writeonly", By(&["facts_of_a_call_argument_are_its_call_site_attributes"])),
    ("noreturn", By(&["test_direct_noreturn_summary_prunes_only_the_callers_impossible_tail"])),
    ("nounwind", By(&["a_routine_that_raises_no_error_is_nounwind"])),
    ("willreturn", Unread("ow-frontend-contract: no frontend states it; functionattrs infers it (test for the inference only)")),
    ("nocallback", Unread("ow-frontend-contract: stated by the QB runtime table; a test of globalsaa reading it is owed")),
    ("cold", By(&["test_a_frontend_cold_block_stays_cold_in_the_rich_mir"])),
    ("threeway", Unread("qcport-rich: a pass that folds a compare of a three-way result")),
    ("dereferenceable", By(&["test_a_reference_parameter_is_dereferenceable"])),
    ("align", By(&["test_alignment_is_the_object_s_less_what_each_index_may_add", "an_alignment_of_no_object_is_refused"])),
    ("initializes", By(&["facts_of_a_call_argument_are_its_call_site_attributes", "test_harr_initializes_the_reused_counter_before_its_exit_bound"])),
    ("memory", By(&["a_readnone_or_memory_none_callee_touches_nothing"])),
    ("nsw", By(&["a_no_signed_wrap_fact_is_nsw"])),
    ("nuw", Unread("ow-frontend-contract: Nib range loops state it; a loop pass that needs it (scev) is owed")),
    ("inbounds", By(&["inbounds_is_a_fact_of_an_operand"])),
    ("inline", By(&["test_a_callee_the_language_says_never_inline_stays_even_if_always_is_stated_too", "test_an_inline_hint_raises_the_budget_by_llvms_ratio_and_not_for_size", "each_inlining_has_its_attribute"])),
    ("reassoc", By(&["floating_freedoms_are_fast_math_flags", "test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation"])),
    ("nnan", By(&["floating_freedoms_are_fast_math_flags", "test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation"])),
    ("ninf", By(&["floating_freedoms_are_fast_math_flags", "test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation"])),
    ("nsz", By(&["floating_freedoms_are_fast_math_flags", "test_nsz_lets_a_cancellation_be_positive_zero"])),
    ("arcp", By(&["floating_freedoms_are_fast_math_flags", "test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation"])),
];

/// The facts a row is owed for: every one the table declares.
fn declared() -> Vec<&'static str> {
    Fact::examples().into_iter().map(Fact::key).collect()
}

/// Whether some source under the workspace defines `fn name`.
fn defined(name: &str, files: &[String]) -> bool {
    let needle = format!("fn {name}(");
    files.iter().any(|text| text.contains(&needle))
}

fn sources() -> Vec<String> {
    let mut found = Vec::new();
    let mut directories = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory).expect("a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "target") {
                    directories.push(path);
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    found
}

/// A fact added without a row, a row naming a test that is gone, and a row
/// for a fact that is gone, were each invisible until a pass misread the
/// fact; now they fail here.
#[test]
fn every_fact_is_shown_end_to_end_or_says_whose_reader_waits() {
    let files = sources();
    let mut wrong = Vec::new();
    for key in declared() {
        match COVERAGE.iter().find(|(name, _)| *name == key) {
            None => wrong.push(format!("{key}: no row in COVERAGE")),
            Some((_, By(tests))) => wrong.extend(tests.iter().filter(|test| !defined(test, &files)).map(|test| format!("{key}: no `fn {test}`"))),
            Some((_, Unread(why))) if why.is_empty() => wrong.push(format!("{key}: says nothing of its reader")),
            Some(_) => {}
        }
    }
    wrong.extend(COVERAGE.iter().filter(|(name, _)| !declared().contains(name)).map(|(name, _)| format!("{name}: a row for no fact")));
    assert_eq!(wrong, Vec::<String>::new());
}
