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
    ("noalias", By(&["a_stated_fact_becomes_its_carrier", "test_restrict_parameters_are_noalias", "noalias_parameters_are_disjoint_and_plain_ones_may_alias", "test_a_noalias_result_is_apart_from_every_other_object"])),
    ("readonly", By(&["test_a_load_from_a_readonly_noalias_parameter_leaves_past_a_store"])),
    ("readnone", By(&["a_readnone_or_memory_none_callee_touches_nothing"])),
    ("nonnull", By(&["a_frame_object_is_nonnull_and_a_parameter_is_not"])),
    ("nocapture", By(&["facts_of_a_call_argument_are_its_call_site_attributes", "test_known_capture_summary_controls_later_external_reach"])),
    ("noretain", By(&["a_noretain_argument_publishes_nothing_it_points_to", "a_global_held_only_by_a_noretain_argument_stays_tracked", "noretain_is_a_parameter_attribute_that_round_trips"])),
    ("releases", By(&["releases_is_a_parameter_attribute_that_round_trips", "a_routine_that_frees_a_temporary_is_not_inlined_at_a_call_of_one"])),
    ("writeonly", By(&["facts_of_a_call_argument_are_its_call_site_attributes", "a_write_only_argument_publishes_nothing_it_will_hold"])),
    ("noreturn", By(&["test_a_stated_noreturn_callee_ends_the_path"])),
    ("nounwind", By(&["a_routine_that_raises_no_error_is_nounwind", "purity_refuses_nontermination_nonlocal_accesses_and_what_callees_do_not_state"])),
    ("willreturn", Unread("ow-frontend-contract: no frontend states it; functionattrs infers it (test for the inference only)")),
    ("norecurse", By(&["the_stamp_infers_norecurse_where_nothing_can_reenter", "test_a_leaf_function_comes_out_of_the_compile_norecurse"])),
    ("mustprogress", By(&["the_stamp_takes_the_languages_word_that_a_loop_ends", "a_loop_with_no_exit_is_never_taken_to_end"])),
    ("nocallback", Unread("ow-frontend-contract: stated by the QB runtime table; a test of globalsaa reading it is owed")),
    ("cold", By(&["test_a_frontend_cold_block_stays_cold_in_the_rich_mir"])),
    ("threeway", Unread("qcport-rich: a pass that folds a compare of a three-way result")),
    ("stackcheck", By(&["the_limit_and_handler_come_from_the_runtime_description", "test_a_small_leaf_the_callers_check_covers_goes_unchecked"])),
    ("dereferenceable", By(&["test_a_reference_parameter_is_dereferenceable"])),
    ("align", Unread("ow-frontend-contract: valuetracking reads a data object's Align (tested in llrm-mir); a pass-level test through a load fold or hoist is owed")),
    ("initializes", By(&["facts_of_a_call_argument_are_its_call_site_attributes", "a_call_filling_a_buffer_kills_its_earlier_fill"])),
    ("memory", By(&["a_readnone_or_memory_none_callee_touches_nothing"])),
    ("nsw", By(&["a_no_signed_wrap_fact_is_nsw", "test_a_step_promised_not_to_wrap_ends_an_inclusive_symbolic_loop"])),
    ("nuw", By(&["test_a_step_promised_not_to_wrap_ends_an_inclusive_symbolic_loop"])),
    ("inbounds", By(&["inbounds_is_a_fact_of_an_operand", "test_an_inequality_loop_is_bounded_by_its_in_bounds_accesses"])),
    ("inline", By(&["test_a_callee_the_language_says_never_inline_stays_even_if_always_is_stated_too", "test_an_inline_hint_raises_the_budget_by_llvms_ratio_and_not_for_size", "each_inlining_has_its_attribute"])),
    ("reassoc", By(&["test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation"])),
    ("nnan", By(&["test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation"])),
    ("ninf", By(&["test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation"])),
    ("nsz", By(&["floating_freedoms_are_fast_math_flags", "test_nsz_lets_a_cancellation_be_positive_zero"])),
    ("range", By(&["a_stated_range_bounds_a_parameter_and_a_call_result", "a_range_in_metadata_bounds_a_load_and_a_call_result", "a_range_a_frontend_states_of_an_instruction_bounds_its_result", "a_matched_enums_tag_is_bounded_by_its_variants_where_ranges_reads_it"])),
    ("arcp", By(&["test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation"])),
    ("invariant", By(&["a_load_the_language_says_is_invariant_is_reused_across_a_call_that_may_write", "test_a_load_the_language_says_is_invariant_leaves_past_a_store", "a_loads_invariance_is_read_from_its_metadata"])),
    ("unroll", By(&["a_loop_the_language_says_not_to_unroll_stays_rolled", "a_loop_the_language_permits_is_copied_past_the_budget_and_the_cap", "a_loops_unroll_hint_is_read_from_its_metadata"])),
];

/// The facts a row is owed for: every one the table declares.
fn declared() -> Vec<&'static str> {
    Fact::examples().into_iter().map(Fact::key).collect()
}

/// The files under the workspace that define `fn name`, as paths from `crates/`.
fn defined(name: &str, files: &[(String, String)]) -> Vec<String> {
    let needle = format!("fn {name}(");
    files.iter().filter(|(_, text)| text.contains(&needle)).map(|(path, _)| path.clone()).collect()
}

/// Whether a test at `path` shows a pass of the compile route reading the fact:
/// the optimiser, the back end or a frontend through the driver. Not the IR
/// crates, whose `llrm_mir::transforms` the compile route does not run (#237).
fn on_the_compile_route(path: &str) -> bool {
    path.starts_with("opt/") || path.starts_with("backend/") || path.starts_with("frontends/")
}

fn sources() -> Vec<(String, String)> {
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
                let relative = path.strip_prefix(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap_or(&path).to_string_lossy().replace("/../../", "/").to_string();
                found.push((relative, std::fs::read_to_string(&path).unwrap_or_default()));
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
            Some((_, By(tests))) => {
                wrong.extend(tests.iter().filter(|test| defined(test, &files).is_empty()).map(|test| format!("{key}: no `fn {test}`")));
                // A reader that only an IR crate's tests exercise has shown nothing about the compile.
                if !tests.iter().any(|test| defined(test, &files).iter().any(|path| on_the_compile_route(path))) {
                    wrong.push(format!("{key}: no test on the compile route (opt/, backend/, frontends/); name one or say `Unread`"));
                }
            }
            Some((_, Unread(why))) if why.is_empty() => wrong.push(format!("{key}: says nothing of its reader")),
            Some(_) => {}
        }
    }
    wrong.extend(COVERAGE.iter().filter(|(name, _)| !declared().contains(name)).map(|(name, _)| format!("{name}: a row for no fact")));
    assert_eq!(wrong, Vec::<String>::new());
}
