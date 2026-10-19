//! JumpThreading against LLVM's `jump-threading` on `tests/jumpthreading/*.ll`,
//! one fixture per shape of stages S1 to S3, and the boundary of each DEVIATE
//! of `jumpthreading.rs`.
//!
//! Each `NAME.ll` is run through the pass and must come out as
//! `NAME.after.ll` (written by `LLRM_BLESS=1`); `tools/difftest/difftest.py`
//! reads the same pair against `opt -passes=jump-threading`.

use std::path::{Path, PathBuf};

use llrm_mir::target::Neutral;

use crate::jumpthreading::JumpThreading;
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
    let mut module = parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(JumpThreading { enabled: true });
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

/// JumpThreading stage 1 ran away on datetime.c -Os (50 GB in 100 s) while on
/// by default: unset, the pass is off.
#[test]
fn test_jumpthreading_is_off_unless_the_env_asks() {
    assert_eq!(JumpThreading::from_env().enabled, std::env::var_os("LLRM_JUMPTHREADING").is_some());
}
