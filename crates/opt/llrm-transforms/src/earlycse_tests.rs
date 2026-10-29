//! Tests of the EarlyCSE translation (`early-cse<memssa>`): what its scoped
//! tables and generation count do.

use llrm_analysis::testing::DOS;

use super::EarlyCse;
use crate::testing::{managed, parsed};

fn run(body: &str) -> String {
    managed(&mut parsed(&format!("{DOS}{body}")), EarlyCse::default())
}

/// A commuted add was a second add, which `gvn` found much later.
#[test]
fn test_a_commuted_operation_is_the_one_before() {
    let after = run("define i32 @f(i32 %x, i32 %y) {
b0:
  %a = add i32 %x, %y
  %b = add i32 %y, %x
  %c = mul i32 %a, %b
  ret i32 %c
}
");
    assert_eq!(after.matches("add i32").count(), 1, "{after}");
}

/// A compare with its operands exchanged and the predicate swapped is the same
/// value.
#[test]
fn test_a_swapped_compare_is_the_one_before() {
    let after = run("define i1 @f(i32 %x, i32 %y) {
b0:
  %a = icmp slt i32 %x, %y
  %b = icmp sgt i32 %y, %x
  %c = and i1 %a, %b
  ret i1 %c
}
");
    assert_eq!(after.matches("icmp").count(), 1, "{after}");
}

/// A select on the inverted condition with its arms exchanged is the same
/// value.
#[test]
fn test_an_inverted_select_is_the_one_before() {
    let after = run("define i32 @f(i32 %x, i32 %y, i32 %p, i32 %q) {
b0:
  %c1 = icmp slt i32 %x, %y
  %c2 = icmp sge i32 %x, %y
  %a = select i1 %c1, i32 %p, i32 %q
  %b = select i1 %c2, i32 %q, i32 %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("select").count(), 1, "{after}");
}

/// A load of a cell just stored is the stored value.
#[test]
fn test_a_load_after_a_store_is_the_stored_value() {
    let after = run("define i32 @f(ptr %p, i32 %v) {
b0:
  store i32 %v, ptr %p
  %l = load i32, ptr %p
  ret i32 %l
}
");
    assert!(!after.contains("load") && after.contains("ret i32 %v"), "{after}");
}

/// Two loads with no write between are one.
#[test]
fn test_a_second_load_is_the_first() {
    let after = run("define i32 @f(ptr %p) {
b0:
  %a = load i32, ptr %p
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("load").count(), 1, "{after}");
}

/// Of two stores to a cell with nothing reading it between, the first is dead.
#[test]
fn test_a_store_overridden_by_the_next_is_removed() {
    let after = run("define void @f(ptr %p) {
b0:
  store i32 1, ptr %p
  store i32 2, ptr %p
  ret void
}
");
    assert_eq!(after.matches("store").count(), 1, "{after}");
    assert!(after.contains("store i32 2"), "{after}");
}

/// A call that may write the cell ends what was loaded from it.
#[test]
fn test_a_load_is_not_forwarded_across_a_call_that_may_write() {
    let after = run("declare void @opaque(ptr)

define i32 @f(ptr %p) {
b0:
  %a = load i32, ptr %p
  call void @opaque(ptr %p)
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
");
    assert_eq!(after.matches("load").count(), 2, "{after}");
}

/// The branch taken tells the condition: in the arm a repeat of it is a
/// constant.
#[test]
fn test_a_condition_is_known_in_the_arm_it_chose() {
    let after = run("define i1 @f(i32 %x) {
b0:
  %c = icmp eq i32 %x, 0
  br i1 %c, label %t, label %e

t:
  %d = icmp eq i32 %x, 0
  ret i1 %d

e:
  ret i1 false
}
");
    assert!(after.contains("ret i1 true"), "{after}");
}

/// `CallValue::canHandle` takes a call that only reads or only writes memory.
/// A callee that also touches `inaccessiblemem` is neither, and LLVM's
/// `early-cse<memssa>` keeps both calls of `@a` and `@b` below (checked with
/// the pinned `opt`, 24.0.0git d1106deb71cc) while it takes the second of
/// `@c`'s. `call_key` reads `memory::of`, which counts every location; the
/// narrower `accessible` summary would take `@b` for a reader and `@a` for a
/// call that touches nothing, and drop the second call of each.
#[test]
fn test_a_call_that_touches_inaccessible_memory_is_not_a_read_only_or_write_only_call() {
    let after = run("declare i32 @a(ptr) memory(inaccessiblemem: readwrite)
declare i32 @b(ptr) memory(argmem: read, inaccessiblemem: write)
declare i32 @c(ptr) memory(argmem: read)

define i32 @f(ptr %p) {
b0:
  %a1 = call i32 @a(ptr %p)
  %a2 = call i32 @a(ptr %p)
  %b1 = call i32 @b(ptr %p)
  %b2 = call i32 @b(ptr %p)
  %c1 = call i32 @c(ptr %p)
  %c2 = call i32 @c(ptr %p)
  %s1 = add i32 %a1, %a2
  %s2 = add i32 %b1, %b2
  %s3 = add i32 %c1, %c2
  %s4 = add i32 %s1, %s2
  %s5 = add i32 %s4, %s3
  ret i32 %s5
}
");
    assert_eq!(after.matches("call i32 @a").count(), 2, "{after}");
    assert_eq!(after.matches("call i32 @b").count(), 2, "{after}");
    assert_eq!(after.matches("call i32 @c").count(), 1, "{after}");
}

/// `simplifyInstruction` was a stub of a few identities, so a compare that only
/// known bits decide (`(x & 7) u< 8`) stayed in the function.
#[test]
fn test_a_compare_the_known_bits_decide_is_folded() {
    let after = run("define i1 @f(i32 %x) {
b0:
  %m = and i32 %x, 7
  %c = icmp ult i32 %m, 8
  ret i1 %c
}
");
    assert!(!after.contains("icmp"), "{after}");
}

/// A compare implied by the branch that dominates it is the branch's answer
/// (`isImpliedByDomCondition`, which needs the dominator tree in the query).
#[test]
fn test_a_compare_the_dominating_branch_implies_is_folded() {
    let after = run("define i32 @f(i32 %x) {
b0:
  %a = icmp ult i32 %x, 10
  br i1 %a, label %b1, label %b2
b1:
  %c = icmp ult i32 %x, 20
  %r = zext i1 %c to i32
  ret i32 %r
b2:
  ret i32 0
}
");
    assert_eq!(after.matches("icmp").count(), 1, "{after}");
}

/// A store in one arm of a diamond before a load, and no write between that
/// load and one after the next join (x_hash's `keys[slot]`): LLVM's walker
/// answers the one `MemoryPhi` for the later load, and it dominates the
/// earlier load's access. The manager's walk answered a set of nearest defs
/// ({live-on-entry, the store}); a def that did not dominate kept the second
/// load, 524 B instead of 515.
#[test]
fn test_a_load_after_a_join_is_the_one_before_it_when_a_phi_is_the_clobber() {
    let after = run("define i32 @f(ptr %p, i1 %c, i1 %d) {
b0:
  br i1 %c, label %b1, label %b2
b1:
  store i32 1, ptr %p
  br label %b3
b2:
  br label %b3
b3:
  %x = load i32, ptr %p
  br i1 %d, label %b4, label %b5
b4:
  br label %b6
b5:
  br label %b6
b6:
  %y = load i32, ptr %p
  %r = add i32 %x, %y
  ret i32 %r
}
");
    assert_eq!(after.matches("load").count(), 1, "{after}");
}

/// Asks for the manager's MemorySSA and says nothing else; `forgets` returns
/// `none()`, as a pass that edited without naming what it kept.
struct AsksMssa {
    forgets: bool,
}

impl llrm_mir::passes::FunctionPass for AsksMssa {
    fn name(&self) -> &'static str {
        "asks-mssa"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<llrm_analysis::manager::MemorySsa>(unit.context, unit.layout, unit.function);
        if self.forgets {
            llrm_mir::passes::PreservedAnalyses::none()
        } else {
            llrm_mir::passes::PreservedAnalyses::all()
        }
    }
}

const ONE_LOAD_TWICE: &str = "define i32 @f(ptr %p) {
b0:
  %a = load i32, ptr %p
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
";

fn mssa_misses_over(passes: Vec<Box<dyn llrm_mir::passes::FunctionPass>>) -> (u64, u64) {
    let mut module = parsed(&format!("{DOS}{ONE_LOAD_TWICE}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.verify_invalidation = true;
    for pass in passes {
        manager.add_boxed(pass);
    }
    let before = llrm_mir::counts::analysed("mssa");
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    let after = llrm_mir::counts::analysed("mssa");
    (after.0 - before.0, after.1 - before.1)
}

/// EarlyCSE removes the second load through `MemorySSAUpdater` and says it kept
/// MemorySSA (EarlyCSE.cpp `PA.preserve<MemorySSAAnalysis>()`): the graph a
/// pass asks for after it is the one it updated, not a new one. The manager
/// checks it against a fresh build (`verify_invalidation`). Before, EarlyCSE
/// built its own graph per body and the manager's was dropped with its change:
/// the pass after it computed MemorySSA again (2 computations, not 1).
#[test]
fn test_the_memory_graph_early_cse_updated_is_the_one_the_next_pass_gets() {
    let (hits, misses) = mssa_misses_over(vec![
        Box::new(AsksMssa { forgets: false }),
        Box::new(EarlyCse::default()),
        Box::new(AsksMssa { forgets: false }),
    ]);
    assert_eq!(misses, 1, "MemorySSA was computed again after EarlyCSE updated it");
    assert!(hits >= 2, "EarlyCSE and the pass after it read the one graph: {hits} hits");
}

/// A pass that edits and keeps nothing still drops it: the graph after is new.
#[test]
fn test_the_memory_graph_is_computed_again_after_a_pass_that_keeps_nothing() {
    let (_, misses) =
        mssa_misses_over(vec![Box::new(AsksMssa { forgets: true }), Box::new(AsksMssa { forgets: false })]);
    assert_eq!(misses, 2, "a pass that named nothing kept MemorySSA");
}

/// EarlyFPM's EarlyCSE has no MemorySSA (`EarlyCSEPass()`,
/// PassBuilderPipelines.cpp#L1180; `UseMemorySSA` false, EarlyCSE.cpp#L1815):
/// it asks for no graph. Built anyway, d_faces -O2 built MemorySSA 11 times
/// more than the pin's one per function (22.7 M of 42.3 M in EarlyCSE).
#[test]
fn test_early_cse_without_memory_ssa_builds_no_graph() {
    let (hits, misses) = mssa_misses_over(vec![Box::new(EarlyCse { memory_ssa: false })]);
    assert_eq!((hits, misses), (0, 0), "MemorySSA was asked for");
    let (hits, misses) = mssa_misses_over(vec![Box::new(EarlyCse::default())]);
    assert!(hits + misses > 0, "premise: with MemorySSA it asks for the graph");
}

/// Without MemorySSA a load past a write is a new load (`isSameMemGeneration`
/// is false unless the generations are equal, EarlyCSE.cpp#L1074); with it a
/// store to a cell that cannot alias the loaded one is not in the way.
#[test]
fn test_without_memory_ssa_a_load_past_a_store_to_another_cell_is_not_the_first() {
    let body = "define i32 @f(ptr noalias %p, ptr noalias %q) {
b0:
  %a = load i32, ptr %p
  store i32 7, ptr %q
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
";
    let with = managed(&mut parsed(&format!("{DOS}{body}")), EarlyCse::default());
    let without = managed(&mut parsed(&format!("{DOS}{body}")), EarlyCse { memory_ssa: false });
    assert_eq!(with.matches("load").count(), 1, "premise: with MemorySSA the second load goes:\n{with}");
    assert_eq!(without.matches("load").count(), 2, "{without}");
}

/// A load of a constant table is the one before a call that may write
/// anything (`pointsToConstantMemory`, `opt` agrees). QCport's
/// `config.c` kept the second `load [_fields + i + 4]` after a `strncpy`
/// when EarlyCSE read unannotated accesses: +34 B over three modules.
#[test]
fn test_a_load_of_a_constant_table_survives_a_call() {
    let after = run("@fields = internal constant [4 x i16] [i16 1, i16 2, i16 3, i16 4]

declare ptr @strncpy_(ptr, ptr, i16)

define i16 @f(ptr noundef %base, ptr noundef %src, i16 %i) {
b0:
  %p = getelementptr inbounds i16, ptr @fields, i16 %i
  %a = load i16, ptr %p
  %d = getelementptr inbounds i8, ptr %base, i16 %a
  %c = call ptr @strncpy_(ptr %d, ptr %src, i16 63)
  %b = load i16, ptr %p
  %r = add i16 %a, %b
  ret i16 %r
}
");
    assert_eq!(after.matches("load i16").count(), 1, "{after}");
}

/// EarlyCSE cloned the whole context to give MemorySSA a view the walk's
/// interned constants did not reach (`Context::int` interns through `&self`, so
/// the copy bought nothing): 0.7 M Ir over 22 runs of d_faces, plus its drop.
#[test]
fn test_a_run_clones_no_context() {
    use llrm_mir::counts::{Event, get};
    let before = get(Event::ContextClones);
    let after = run("define i32 @f(i1 %c, i32 %x) {
b0:
  br i1 %c, label %a, label %b
a:
  %p = add i32 %x, 1
  ret i32 %p
b:
  %q = add i32 %x, 1
  ret i32 %q
}
");
    assert!(after.contains("define i32 @f"), "{after}");
    assert_eq!(get(Event::ContextClones), before, "earlycse cloned the context");
}

/// Two loads with equal `!range` nodes were one load that lost its range, so
/// `ipsccp` found no argument range for quicksort's `sort` (+30% clocks).
#[test]
fn test_a_load_cse_keeps_the_range_both_loads_carry() {
    let after = run("define i16 @f(ptr %p) {
b0:
  %a = load i16, ptr %p, !range !0
  %b = load i16, ptr %p, !range !1
  %c = add i16 %a, %b
  ret i16 %c
}
!0 = !{i16 0, i16 100}
!1 = !{i16 0, i16 100}
");
    assert_eq!(after.matches("load").count(), 1, "{after}");
    assert!(after.contains("!range"), "{after}");
}
