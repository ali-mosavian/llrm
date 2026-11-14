//! DEVIATE UNROLL-SPEED-TRIAL: at -O2 and -O3 the driver keeps the unroll variant whose module is smaller in bytes.
//! loop_fusion -O2 was 1623 B / 208734 T (the loops left rolled, SDCC 901 B) and memmove_overlap -O3 1329 B / 27277 T
//! (`fill`, 32 trips, unrolled and inlined; 589 B rolled).

use llrm_z80::compile::{effective, module_prices};
use llrm_transforms::trial::SpeedUnroll;
use llrm_z80::driver::{
    Fingerprint, GROWN, Level, Request, UNCHANGED, mid_end_fingerprint, selections_with_alternatives, speed_unrolls_made,
    take_mid_end_runs, with_unroll_base,
};

const LOOP_FUSION: &str = include_str!("data/loop_fusion_o2.ll");
const MEMMOVE: &str = include_str!("data/memmove_overlap_o3.ll");

/// loop_fusion -O2: a full unroll that the pin's threshold refuses folds the loads of the next loop and deletes an
/// array; the module made with the -O3 threshold is smaller by hundreds of bytes and the driver takes it.
#[test]
fn a_wider_unroll_is_taken_where_the_module_comes_out_smaller() {
    let rolled = Request::new(Level::O2);
    let chosen = effective(LOOP_FUSION, &rolled).expect("chooses");
    assert!(chosen.unroll_variant.is_some(), "the pin's unroll kept at -O2");
    let (before, after) = (module_prices(LOOP_FUSION, &rolled).unwrap().0, module_prices(LOOP_FUSION, &chosen).unwrap().0);
    assert!(after + 500 <= before, "{before} B kept as {after} B");
}

/// memmove_overlap -O3: the pin unrolls `fill` and the inliner copies it into `main`; the variant that unrolls only
/// what adds no bytes is 740 B smaller.
#[test]
fn an_unroll_that_only_grows_the_module_is_refused() {
    let unrolled = Request::new(Level::O3);
    let chosen = effective(MEMMOVE, &unrolled).expect("chooses");
    assert!(chosen.unroll_variant.is_some(), "the pin's unroll kept at -O3");
    let (before, after) = (module_prices(MEMMOVE, &unrolled).unwrap().0, module_prices(MEMMOVE, &chosen).unwrap().0);
    assert!(after + 500 <= before, "{before} B kept as {after} B");
}

/// The trial compiles a variant only where it can differ: a loop refused (known trips) is what the wider threshold
/// could unroll and a loop unrolled is what the no-growth variant could undo. -O1, -Os and -Oz keep the pin's
/// decisions and note none.
#[test]
fn a_variant_is_compiled_only_where_a_decision_could_change() {
    selections_with_alternatives(LOOP_FUSION, &Request::new(Level::O2)).expect("selects");
    assert!(speed_unrolls_made().refused > 0, "loop_fusion -O2 refuses no unroll: {:?}", speed_unrolls_made());
    selections_with_alternatives(MEMMOVE, &Request::new(Level::O3)).expect("selects");
    assert!(speed_unrolls_made().made > 0, "memmove_overlap -O3 unrolls nothing: {:?}", speed_unrolls_made());
    for level in [Level::O1, Level::Os, Level::Oz] {
        let chosen = effective(LOOP_FUSION, &Request::new(level)).expect("chooses");
        assert!(chosen.unroll_variant.is_none(), "{level:?} chose {:?}", chosen.unroll_variant);
        selections_with_alternatives(LOOP_FUSION, &Request::new(level)).expect("selects");
        assert_eq!(speed_unrolls_made().made + speed_unrolls_made().refused, 0, "{level:?} noted decisions");
    }
}

/// A variant whose mid-end makes the module the base compile made (the -O3 threshold is the one -O3 has) is not
/// selected: its compile stops at the mid-end, where the whole compile of it would give the same object.
#[test]
fn a_variant_that_leaves_the_module_as_it_is_is_not_selected() {
    selections_with_alternatives(LOOP_FUSION, &Request::new(Level::O3)).expect("selects");
    let base = mid_end_fingerprint();
    assert!(base.is_some(), "no fingerprint of a module with unroll decisions");
    let same = Request { unroll_variant: Some(SpeedUnroll::Wide(300)), ..Request::new(Level::O3) };
    let made = with_unroll_base(base, || selections_with_alternatives(LOOP_FUSION, &same));
    assert_eq!(made.err().as_deref(), Some(UNCHANGED));
}

/// A variant that adds more than `UNROLL_TRIAL_GROWTH` instructions to the module is not selected either (fcmp_all
/// -O2: 7803 instructions, 9879 with the wide unroll, 32733 B against 17239 B; the selection of it was 2/3 of a
/// trial that cost 3x the compile).
#[test]
fn a_variant_that_grows_the_module_far_is_not_selected() {
    let narrow = Request { unroll_variant: Some(SpeedUnroll::NoGrow), ..Request::new(Level::O2) };
    let start = Fingerprint { hash: 0, instructions: 0 };
    with_unroll_base(Some(start), || selections_with_alternatives(LOOP_FUSION, &narrow)).expect("selects");
    let base = mid_end_fingerprint();
    assert!(base.is_some_and(|base| base.instructions > 0));
    let wide = Request { unroll_variant: Some(SpeedUnroll::Wide(300)), ..Request::new(Level::O2) };
    let made = with_unroll_base(base, || selections_with_alternatives(LOOP_FUSION, &wide));
    assert_eq!(made.err().as_deref(), Some(GROWN), "growth {:?} instructions", base);
}

/// The module's data text came from a second run of the whole mid-end, once for pricing and again for emitting.
/// One run of the mid-end now serves a module's code and its data.
#[test]
fn a_modules_code_and_data_are_made_by_one_mid_end_run() {
    let request = Request::new(Level::O1);
    take_mid_end_runs();
    llrm_z80::compile::text(LOOP_FUSION, "main", 0, &request).expect("compiles");
    llrm_z80::data::text(LOOP_FUSION, &request).expect("data");
    assert_eq!(take_mid_end_runs(), 1);
}

/// huffman_decode -O3 (`-fno-inline-functions`): the no-growth variant left the nests to the pin's -O3 threshold and
/// kept the unrolled `build`/`decode` loops, 1085 B in the module price where the same variant at -O2 is 698 B (the
/// cell was 1035 B against SDCC's 719). The variant prices every nest, at any level.
#[test]
fn the_no_growth_variant_prices_nests_at_o3() {
    let ir = include_str!("data/huffman_decode_o3.ll");
    let chosen = effective(ir, &Request::new(Level::O3)).expect("chooses");
    let bytes = module_prices(ir, &chosen).unwrap().0;
    assert!(bytes <= 750, "{bytes} B kept at -O3 (-O2: 698)");
}

const MUL_WORDS: &str = include_str!("kernels/ir/mul_words_to_long.ll");

/// mul_words_to_long -O3: the pin unrolls the 10-trip loop of calls whose table loads fold (207 B; 137 B rolled, SDCC
/// 197 B). The no-growth variant priced the copies at 0 B: the analysis had folded the table's loads out of the bytes
/// it counts and the promoted-load credit took them out again (51 B added, priced 0).
#[test]
fn an_unroll_whose_table_loads_are_already_folded_is_not_credited_twice() {
    let unrolled = Request::new(Level::O3);
    let chosen = effective(MUL_WORDS, &unrolled).expect("chooses");
    let (before, after) = (module_prices(MUL_WORDS, &unrolled).unwrap().0, module_prices(MUL_WORDS, &chosen).unwrap().0);
    assert!(after + 40 <= before, "{before} B kept as {after} B");
}
