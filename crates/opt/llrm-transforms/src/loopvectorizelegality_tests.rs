//! `can_vectorize` against the pinned LLVM's `opt -passes=loop-vectorize
//! -debug-only=loop-vectorize` verdict ("We can vectorize this loop!" or
//! "Not vectorizing") on `tests/lv/*.ll`.

use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::loopaccessinfo::LoopAccessInfo;
use llrm_analysis::memory::Unit;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::valuetracking::AssumptionCache;

use super::{Refusal, can_vectorize};

/// Our verdict on the innermost loop of the one function `text` defines.
fn verdict(text: &str) -> Result<super::Legality, Refusal> {
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let (_, _, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("a function");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("a layout");
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let unit = Unit::of(&module, &layout, function);
    let mut se = ScalarEvolution::new();
    let the_loop = loops.get_loops_in_preorder().into_iter().find(|one| one.is_innermost()).expect("a loop");
    let mut aa = BasicAA::new(&unit);
    let lai = LoopAccessInfo::new(&mut se, &cx, the_loop.get_header(), &mut aa, false, u32::MAX);
    can_vectorize(&cx, &mut se, &lai, the_loop)
}

macro_rules! corpus {
    ($($name:ident => $legal:expr,)*) => {
        $(
            #[test]
            fn $name() {
                let found = verdict(include_str!(concat!("../tests/lv/", stringify!($name), ".ll")));
                assert_eq!(found.is_ok(), $legal, "{found:?}");
            }
        )*
    };
}

// The pin's verdicts, in the comment of each: `opt -passes=loop-vectorize
// -debug-only=loop-vectorize`.
corpus! {
    // We can vectorize this loop!
    copy => true,
    // We can vectorize this loop! (a symbolic exact trip count)
    symbolic_trip => true,
    // We can vectorize this loop! (the store to the invariant address is
    // skipped: not a reduction's)
    invariant_store => true,
    // We can vectorize this loop!
    reduction => true,
    // Not vectorizing: Found a non-intrinsic callsite
    with_call => false,
    // vectorized loop (an `llvm.smax` call is trivially vectorizable)
    with_intrinsic => true,
    // vectorized loop (`ctlz`'s second operand is a constant: scalar in every
    // lane)
    with_ctlz => true,
    // loop not vectorized: call instruction cannot be vectorized (`sdiv.fix` is
    // not in `isTriviallyVectorizable`)
    with_sdiv_fix => false,
    // vectorized loop (plain pointers: the pin guards them
    // with `vector.memcheck`)
    memcheck_copy => true,
    // We can vectorize this loop! (`llvm.fmuladd` is trivially vectorizable)
    fp_fmuladd => true,
    // Can't vectorize due to memory conflicts
    dependence => false,
    // Not vectorizing: Cannot vectorize uncountable loop.
    uncountable => false,
}

/// The canonical induction of a copy loop is its `i`, of the widest type (i64
/// here).
#[test]
fn a_zero_based_unit_step_induction_is_the_primary() {
    let found = verdict(include_str!("../tests/lv/copy.ll")).expect("legal");
    assert!(found.primary_induction.is_some());
    assert_eq!(found.widest_induction_bits, Some(64));
}

/// `differ` of memory_expansion: a byte compare that leaves at the first
/// difference. The pin's legality takes it (`isVectorizableEarlyExitLoop`, the
/// loads dereferenceable in the loop); every loop of two blocks was refused,
/// so clang's `pcmpeqb` loop stayed a scalar one (1.69 M instructions against
/// 0.93 M).
const DIFFER: &str = "target datalayout = \"e-m:e-p:64:64-i64:64-n8:16:32:64\"

@a = global [1200 x i8] zeroinitializer
@b = global [1200 x i8] zeroinitializer

define i32 @f() {
entry:
  br label %body
body:
  %i = phi i64 [ 0, %entry ], [ %i.next, %latch ]
  %pa = getelementptr inbounds i8, ptr @a, i64 %i
  %pb = getelementptr inbounds i8, ptr @b, i64 %i
  %x = load i8, ptr %pa, align 1
  %y = load i8, ptr %pb, align 1
  %ne = icmp ne i8 %x, %y
  br i1 %ne, label %found, label %latch
latch:
  %i.next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %i.next, 1200
  br i1 %done, label %same, label %body
found:
  ret i32 1
same:
  ret i32 0
}
";

#[test]
fn a_byte_compare_that_leaves_early_is_legal_with_an_early_exit() {
    let found = verdict(DIFFER).expect("legal");
    let early = found.early_exit.expect("an early-exit loop");
    assert_eq!((early.uncountable.len(), early.countable.len()), (1, 1));
}

/// The same loop over a global too small for the walk: the load may fault, so
/// it is not legal (`NonReadOnlyEarlyExitLoop`).
#[test]
fn an_early_exit_loop_that_may_fault_is_refused() {
    let small = DIFFER.replace("[1200 x i8]", "[1000 x i8]");
    assert!(verdict(&small).is_err());
}
