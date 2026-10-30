//! JumpThreading against LLVM's `jump-threading` on `tests/jumpthreading/*.ll`,
//! one fixture per shape of stages S1 to S3, and the boundary of each DEVIATE
//! of `jumpthreading.rs`.
//!
//! Each `NAME.ll` is run through the pass and must come out as
//! `NAME.after.ll` (written by `LLRM_BLESS=1`); `tools/difftest/difftest.py`
//! reads the same pair against `opt -passes=jump-threading`.

use std::path::{Path, PathBuf};

use llrm_mir::interpret::Val;
use llrm_mir::target::Neutral;

use crate::jumpthreading::{JumpThreading, JumpThreadingOptions};
use crate::testing::parsed;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/jumpthreading")
}

/// Every input under `tests/jumpthreading`, with its `.after.ll`.
fn fixtures() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root())
        .expect("tests/jumpthreading")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().expect("a name").to_string_lossy().into_owned();
            name.ends_with(".ll") && !name.ends_with(".after.ll")
        })
        .collect();
    found.sort();
    found
}

/// `text` through the pass under the pass manager, printed; the module must
/// verify.
fn run(text: &str) -> String {
    run_with(text, JumpThreadingOptions::default())
}

fn run_with(
    text: &str,
    options: JumpThreadingOptions,
) -> String {
    let mut module = parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(JumpThreading { enabled: true, options });
    manager.run_module(&mut module, std::rc::Rc::new(Neutral)).unwrap();
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
    llrm_mir::print::module(&module)
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(root().join(format!("{name}.ll"))).expect("a fixture")
}

#[test]
fn every_fixture_comes_out_as_its_after_file() {
    let fixtures = fixtures();
    assert!(fixtures.len() >= 16, "{} fixtures", fixtures.len());
    for path in fixtures {
        let before = std::fs::read_to_string(&path).expect("readable");
        let after = run(&before);
        let expected = path.with_extension("after.ll");
        if std::env::var_os("LLRM_BLESS").is_some() {
            std::fs::write(&expected, &after).expect("writable");
        }
        let want = std::fs::read_to_string(&expected).unwrap_or_else(|_| panic!("{} has no .after.ll", path.display()));
        assert_eq!(after, want, "{}", path.display());
    }
}

/// Each predecessor of the join knows the branch: nothing reaches the `phi`
/// and its branch, which went from 2 predecessors to none, and no call of
/// `@sink` is lost.
#[test]
fn a_branch_on_a_phi_of_constants_is_threaded_from_each_predecessor() {
    let after = run(&fixture("phi_of_constants"));
    assert!(!after.contains("phi i1"), "the phi of constants is gone:\n{after}");
    assert_eq!(after.matches("call void @sink").count(), 2, "{after}");
}

/// Only `%p1` knows `%c`: `%p2` keeps the test, so a phi stays.
#[test]
fn only_the_predecessor_that_knows_the_condition_skips_the_test() {
    let after = run(&fixture("one_edge_known"));
    assert!(after.contains("br i1 %y"), "the test stays for %p2:\n{after}");
    assert!(after.contains(".thread"), "a copy for %p1:\n{after}");
}

/// A block over the size limit is not copied: 8 instructions against 6.
#[test]
fn a_block_over_the_size_limit_is_not_copied() {
    let before = fixture("size_limit");
    let after = run(&before);
    assert!(!after.contains(".thread"), "{after}");
}

/// The same block with `minsize`'s 3 would be refused earlier: a block of 4
/// instructions threads by default and not with `minsize`.
#[test]
fn minsize_lowers_the_limit_to_three() {
    let text = "define i32 @f(i1 %x, i1 %y, i32 %a) {\nentry:\n  br i1 %x, label %p1, label %p2\np1:\n  call void @sink(i32 1)\n  br label %join\np2:\n  call void @sink(i32 2)\n  br label %join\njoin:\n  %c = phi i1 [ true, %p1 ], [ %y, %p2 ]\n  %v1 = add i32 %a, 1\n  %v2 = mul i32 %v1, 3\n  %v3 = add i32 %v2, %a\n  %v4 = mul i32 %v3, 5\n  br i1 %c, label %t, label %e\nt:\n  ret i32 %a\ne:\n  ret i32 %v4\n}\ndeclare void @sink(i32)\n";
    assert!(run(text).contains(".thread"), "4 instructions thread at 6");
    let minsize = text.replace("%a) {", "%a) minsize {");
    assert!(!run(&minsize).contains(".thread"), "4 instructions are over 3");
}

/// The header of a loop is not threaded across: a `phi` of constants in it
/// would turn the loop into two.
#[test]
fn a_loop_header_is_not_threaded_across() {
    let before = fixture("loop_header");
    assert_eq!(run(&before), llrm_mir::print::module(&parsed(&before)));
}

/// `%v` read below the threaded block gets a `phi` of the original and the
/// copy.
#[test]
fn a_value_read_below_the_copied_block_gets_a_phi() {
    let after = run(&fixture("ssa_update"));
    assert!(after.contains("phi i32"), "{after}");
}

/// Folding the branch left `%dead` with no predecessor: it is deleted, and
/// the function verifies.
#[test]
fn a_block_left_with_no_predecessor_is_deleted() {
    let after = run(&fixture("dead_block"));
    assert_eq!(after.lines().filter(|line| line.ends_with(':')).count(), 1, "one block is left:\n{after}");
}

/// What LVI proves on the edge decides the test for that predecessor.
#[test]
fn a_condition_lvi_proves_on_one_edge_is_threaded_from_it() {
    let after = run(&fixture("lvi_on_edge"));
    assert!(after.contains(".thread"), "{after}");
}

/// `%c` is decided on both edges into `%m` (true from the entry, false through
/// `%z`), which `%b` tests again: before S2 the test of `%c` in `%b` stayed.
#[test]
fn a_test_decided_by_the_edge_into_the_block_before_is_threaded_through_both() {
    let after = run(&fixture("two_block"));
    assert!(after.contains("m.thread"), "{after}");
    assert_eq!(after.matches("br i1 %c").count(), 1, "only the entry tests %c:\n{after}");
}

/// A branch on a phi of two compares stayed a phi of `i1` and one branch; it
/// is a branch on each compare, in each predecessor.
#[test]
fn a_branch_on_a_phi_is_copied_into_the_predecessors_that_jump_to_it() {
    let after = run(&fixture("dup_cond_branch"));
    assert!(!after.contains("phi i1"), "{after}");
    assert!(after.contains("br i1 %a, label %t, label %e"), "{after}");
    assert!(after.contains("br i1 %b, label %t, label %e"), "{after}");
}

/// `a < b` holds on the way into `%m`, which implies `a <= b` in `%w`;
/// LazyValueInfo has no range for either side, so the test stayed.
#[test]
fn a_branch_implied_by_a_dominating_branch_is_folded() {
    let after = run(&fixture("implied_condition"));
    assert!(!after.contains("br i1 %d"), "{after}");
    assert!(after.contains("br i1 %t, label %x, label %y"), "{after}");
}

/// The select in `%p1` has an arm that decides the compare in `%bb`: it is
/// expanded to a branch, so that `%p1` can skip the test.
#[test]
fn a_select_whose_arm_decides_the_compare_is_unfolded_to_a_branch() {
    let after = run(&fixture("select_unfold_cmp"));
    assert!(!after.contains("select"), "{after}");
    assert!(after.contains("br i1 %s, label %t, label %bb"), "{after}");
}

/// A select on a phi of constants in its own block is expanded to a branch
/// and a phi.
#[test]
fn a_select_on_a_phi_with_constant_inputs_is_unfolded_in_its_block() {
    let after = run(&fixture("select_in_curr_bb"));
    assert!(!after.contains("select"), "{after}");
    assert!(after.contains("phi i32"), "{after}");
}

/// The xor of a phi that is `true` from `%p1` is copied into `%p1` with the
/// phi's value in place.
#[test]
fn a_branch_on_a_xor_with_a_known_input_is_copied_into_that_predecessor() {
    let after = run(&fixture("branch_on_xor"));
    assert!(after.contains("%z1 = xor i1 true, %y1"), "{after}");
    assert!(!after.contains("phi i1"), "{after}");
}

/// The select in `%p1` feeds a phi that a `switch` reads: it is expanded to a
/// branch, so that `%p1` can skip the switch.
#[test]
fn a_select_feeding_a_switch_is_unfolded_to_a_branch() {
    let after = run(&fixture("select_unfold_switch"));
    assert!(!after.contains("select"), "{after}");
    assert!(after.contains("br i1 %s, label %t, label %bb"), "{after}");
}

/// `%var` is a phi of constants in `%pred` and `%b` compares it: each
/// predecessor of `%pred` knows the compare, so the one that makes it true
/// skips both blocks.
#[test]
fn a_compare_of_a_phi_in_the_block_before_is_evaluated_on_each_edge() {
    let after = run(&fixture("two_block_phi"));
    assert!(after.contains("pred.thread"), "{after}");
    assert!(after.matches("icmp eq i32 %var, 0").count() == 0, "the compare of %var is gone:\n{after}");
}

/// A pad whose test one invoke decides was copied for it: two landing pads in
/// one function, which `ehprepare` refuses ("more than one landing pad"; ten
/// qb programs, `tests/run/qb/resdiv.bas` among them, did not compile).
#[test]
fn a_block_that_starts_with_a_landing_pad_is_not_copied() {
    let after = run("declare void @f()
declare i32 @__gxx_personality_v0(...)

define i32 @t(i1 %x) personality ptr @__gxx_personality_v0 {
entry:
  br i1 %x, label %a, label %b

a:
  invoke void @f() to label %ok unwind label %pad

b:
  invoke void @f() to label %ok unwind label %pad

ok:
  ret i32 0

pad:
  %c = phi i1 [ true, %a ], [ false, %b ]
  %lp = landingpad { ptr, i32 } cleanup
  br i1 %c, label %t, label %e

t:
  ret i32 1

e:
  resume { ptr, i32 } %lp
}
");
    assert_eq!(after.matches("landingpad").count(), 1, "{after}");
}

/// The copy `threadEdge` places after `PredBB` is visited in the same sweep
/// (LLVM walks the live block list), so it is removed as an empty block before
/// `%b59` is reached and `%b59` keeps the one-input `phi` of `%t` that `opt`
/// leaves. Walking a snapshot of the layout reached `%b59` first, merged it
/// into the copy, folded the `phi` and put the constant in `%b58`'s `phi`:
/// one `phi` fewer than `opt` on d_faces -O2 (body 11, decide@1244).
#[test]
fn a_block_placed_after_the_current_one_is_visited_in_the_same_sweep() {
    let after = run(&fixture("new_block_visited"));
    assert!(after.contains("%t3 = phi i16 [ 2, %b50 ]"), "{after}");
}

/// Threading away the only way into a loop leaves the loop, unreachable and
/// with `%c = add %c, 1` in it, as `opt -passes=jump-threading` does (it
/// deletes only a block with no predecessor): datetime.c -Os saw this and
/// `decide` walked the self-reaching chain forever (#2739). Not JumpThreading's
/// fault.
#[test]
fn a_loop_whose_only_way_in_is_threaded_away_is_left_unreachable_as_opt_leaves_it() {
    let after = run(&fixture("unreachable_cycle"));
    assert!(after.contains("%c = add i32 %c, 1"), "{after}");
}

/// JumpThreading was off by default after the datetime.c runaway (#2736);
/// the flip makes it on unless `LLRM_NO_JUMPTHREADING` is set.
#[test]
fn test_jumpthreading_is_on_unless_the_opt_out_is_set() {
    assert_eq!(JumpThreading::from_env().enabled, std::env::var_os("LLRM_NO_JUMPTHREADING").is_none());
}

/// The limits were file constants, so a level or target could not set them;
/// they are the pin's `cl::opt` values by default and a field changes the
/// decision.
#[test]
fn the_options_default_to_the_pins_values_and_a_lowered_limit_blocks_the_thread() {
    let options = JumpThreadingOptions::default();
    assert_eq!(
        (
            options.bb_dup_threshold,
            options.min_size_bb_dup_threshold,
            options.phi_duplicate_threshold,
            options.implication_search_threshold
        ),
        (6, 3, 76, 3)
    );
    let text = "define i32 @f(i1 %x, i1 %y, i32 %a) {\nentry:\n  br i1 %x, label %p1, label %p2\np1:\n  call void @sink(i32 1)\n  br label %join\np2:\n  call void @sink(i32 2)\n  br label %join\njoin:\n  %c = phi i1 [ true, %p1 ], [ %y, %p2 ]\n  %v1 = add i32 %a, 1\n  %v2 = mul i32 %v1, 3\n  br i1 %c, label %t, label %e\nt:\n  ret i32 %a\ne:\n  ret i32 %v2\n}\ndeclare void @sink(i32)\n";
    assert!(run_with(text, options).contains(".thread"), "2 instructions thread at 6");
    let strict = JumpThreadingOptions { bb_dup_threshold: 0, ..options };
    assert!(!run_with(text, strict).contains(".thread"), "2 instructions are over 0");
}

fn int(bits: u128) -> Val {
    Val::Int { bits, width: 32 }
}

/// `f(arguments)` of fixture `name`, interpreted before and after the pass.
fn value_before_and_after(
    name: &str,
    arguments: &[Val],
) -> (Val, Val) {
    let text = fixture(name);
    let ran = |module: &llrm_mir::module::Module| {
        llrm_mir::interpret::run(module, "f", arguments.to_vec(), 1_000_000).expect("the function runs")
    };
    let before = ran(&parsed(&text));
    (before, ran(&parsed(&run(&text))))
}

/// c/logical_value.c -O2/-Os and c/compare_to_int64.c -O2 printed wrong
/// values (#2886): `single_predecessor` was `getUniquePredecessor`, so a
/// block reached twice from one conditional branch (`br %c, %t, %t`, as
/// merging an empty block into its successor leaves) counted as having a
/// single predecessor, `processImpliedCondition` took the branch's
/// condition as true on that edge, and `f(0)` returned 1.
#[test]
fn a_branch_to_one_block_twice_does_not_imply_its_condition() {
    for a in [0, 1] {
        let (before, after) = value_before_and_after("branch_to_one_block_twice", &[int(a)]);
        assert_eq!(after, before, "f({a})");
    }
}

/// The same fault in c/logical_value.c's -O2 main: three phis on one
/// condition. The sum printed was 30921, not 30192.
#[test]
fn three_phis_on_one_condition_keep_each_paths_value_after_threading() {
    let (before, after) = value_before_and_after("logical_value_chain", &[]);
    assert_eq!(after, before);
}

/// Positions `layout_step` looks at while a sweep removes every block it
/// visits, over `n` blocks.
fn removing_sweep_work(n: u32) -> usize {
    use llrm_mir::module::BlockId;

    let mut layout: Vec<BlockId> = (0..n).map(BlockId).collect();
    let mut placed = vec![true; n as usize];
    let (mut at, mut work) = (0, 0);
    while let Some(&block) = layout.get(at) {
        let after = layout.get(at + 1).copied();
        layout.remove(at);
        placed[block.0 as usize] = false;
        at = crate::jumpthreading::layout_step(&layout, |one| placed[one.0 as usize], block, at, after, || work += 1);
    }
    work
}

/// The walk looked a removed block up by scanning the whole layout, so a
/// sweep that removes every block it visits (empty forwarding blocks)
/// cost n^2/2 positions: 4x the blocks was 16x the work.
#[test]
fn a_sweep_that_removes_every_block_looks_at_a_linear_number_of_positions() {
    let (small, large) = (removing_sweep_work(500), removing_sweep_work(2000));
    assert!(large * 2 <= small * 9, "500 blocks looked at {small} positions, 2000 looked at {large}");
}

/// `fixture` through the pass, printed, and the same fixture parsed and
/// printed unchanged.
fn after_and_unchanged(name: &str) -> (String, String) {
    let before = fixture(name);
    (run(&before), llrm_mir::print::module(&parsed(&before)))
}

/// `%p1` holds the value, `%p2` does not: the test stayed on a load that every
/// path executed, so nothing was threaded from `%p1`. Now `%p2` gets the load
/// and `%p1` skips the test.
#[test]
fn a_load_partially_redundant_on_one_predecessor_is_loaded_in_the_other() {
    let after = run(&fixture("load_pre_partial"));
    assert!(after.contains("%x.pr = load i32, ptr %p"), "a load in %p2:\n{after}");
    assert!(after.contains("join.thread:"), "%p1 skips the test:\n{after}");
}

/// Both predecessors hold the value: no load is inserted.
#[test]
fn a_load_available_on_every_predecessor_is_not_loaded_again() {
    let after = run(&fixture("load_pre_all_available"));
    assert!(!after.contains("load"), "{after}");
}

/// The store to `%q` may overwrite `%p`: the load stayed as the program had it,
/// and a value stored before it would have been the wrong one.
#[test]
fn a_store_that_may_alias_ends_the_scan() {
    let (after, before) = after_and_unchanged("load_pre_aliasing_store");
    assert_eq!(after, before);
}

/// A volatile load is read once, wherever its address was stored.
#[test]
fn a_volatile_load_is_not_made_a_phi() {
    let (after, before) = after_and_unchanged("load_pre_volatile");
    assert_eq!(after, before);
}

/// The address is made in the block of the load: nothing in a predecessor
/// can be asked about it.
#[test]
fn a_load_whose_address_is_made_in_its_block_is_left_alone() {
    let (after, before) = after_and_unchanged("load_pre_address_in_block");
    assert_eq!(after, before);
}

/// The address is a `phi` of the block: each predecessor is asked about its
/// own input.
#[test]
fn a_phi_address_is_translated_for_each_predecessor() {
    let after = run(&fixture("load_pre_phi_address"));
    assert!(!after.contains("load"), "both predecessors stored their address:\n{after}");
}

/// Two predecessors lack the value, so one block takes both and holds the one
/// load.
#[test]
fn predecessors_without_the_value_share_one_inserted_load() {
    let after = run(&fixture("load_pre_split_preds"));
    assert_eq!(after.matches("load").count(), 1, "{after}");
}

/// The stored value is a float and the load an `i32`: the phi reads a bitcast
/// made in the predecessor.
#[test]
fn a_value_of_another_type_reaches_the_phi_through_a_bitcast() {
    let after = run(&fixture("load_pre_cast"));
    assert!(after.contains("bitcast float %v to i32"), "{after}");
}
