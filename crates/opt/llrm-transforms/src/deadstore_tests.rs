//! Each body MIR text run by llrm-mir's interpreter before and after.

use llrm_analysis::testing::DOS;

use super::DeadStoreElimination;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn eliminated(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, DeadStoreElimination);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// A store overwritten before any read goes.
#[test]
fn an_overwritten_store_goes() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("store i16 1"), "{after}");
}

/// `isInstructionTriviallyDead` named four opcodes: the call whose value only
/// the overwritten store held stayed.
#[test]
fn the_pure_call_only_a_dead_store_held_goes_with_it() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine internal i16 @pure(i16 %a) memory(none) nounwind willreturn {\nb0:\n  %r = add i16 %a, 1\n  ret i16 %r\n}\n\ndefine i16 @f(i16 %x) {\nb0:\n  %v = call i16 @pure(i16 %x)\n  store i16 %v, ptr @g\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("call i16 @pure"), "{after}");
}

/// A load between the stores reads the first.
#[test]
fn a_store_that_is_read_stays() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  %a = load i16, ptr @g\n  store i16 %x, ptr @g\n  %b = load i16, ptr @g\n  %r = add i16 %a, %b\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(after.contains("store i16 1"), "{after}");
}

/// A word store over half of an earlier dword store leaves it.
#[test]
fn a_partial_overwrite_stays() {
    let after = eliminated(
        "@g = global i32 0\n\ndefine i32 @f(i16 %x) {\nb0:\n  store i32 1, ptr @g\n  store i16 %x, ptr @g\n  %r = load i32, ptr @g\n  ret i32 %r\n}\n",
        &[&[7]],
    );
    assert!(after.contains("store i32 1"), "{after}");
}

/// A store to a local nothing reads goes (the end of the function).
#[test]
fn a_store_to_an_unread_local_goes() {
    let after = eliminated(
        "define i16 @f(i16 %x) {\nb0:\n  %a = alloca i16\n  store i16 %x, ptr %a\n  ret i16 %x\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("store i16"), "{after}");
}

/// A store to a global that is overwritten on only one arm stays.
#[test]
fn a_store_overwritten_on_one_path_stays() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  %c = icmp eq i16 %x, 0\n  br i1 %c, label %b1, label %b2\n\nb1:\n  store i16 2, ptr @g\n  br label %b3\n\nb2:\n  br label %b3\n\nb3:\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[0], &[7]],
    );
    assert!(after.contains("store i16 1"), "{after}");
}

/// A store to a global overwritten on both arms goes.
#[test]
fn a_store_overwritten_on_every_path_goes() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  %c = icmp eq i16 %x, 0\n  br i1 %c, label %b1, label %b2\n\nb1:\n  store i16 2, ptr @g\n  br label %b3\n\nb2:\n  store i16 3, ptr @g\n  br label %b3\n\nb3:\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[0], &[7]],
    );
    assert!(!after.contains("store i16 1"), "{after}");
}

/// A store of the value just loaded from the same cell is a no-op.
#[test]
fn a_store_of_the_loaded_value_goes() {
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  %v = load i16, ptr @g\n  store i16 %v, ptr @g\n  ret i16 %v\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("store i16"), "{after}");
}

/// A memset covered by the stores after it goes with its bytes.
#[test]
fn a_memset_overwritten_by_a_store_goes() {
    let after = eliminated(
        "@g = global [4 x i8] zeroinitializer\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine i16 @f(i16 %x) {\nb0:\n  call void @llvm.memset.p0.i16(ptr @g, i8 0, i16 2, i1 false)\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("llvm.memset.p0.i16(ptr @g"), "{after}");
}

/// bench/scroll at -O3: the blank bottom row became `memset.pattern` (the loop
/// idiom, which the pin does not form) and the `memmove` that follows covers
/// it. The pin's unrolled stores were deleted; the fill stayed, `rep stosw`
/// per pass (+5.5% clocks).
#[test]
fn a_memset_pattern_overwritten_by_a_memmove_goes() {
    let after = eliminated(
        "@g = global [2000 x i16] zeroinitializer
declare void @llvm.experimental.memset.pattern.p0.i16.i32(ptr, i16, i32, i1)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)

define void @f() {
b0:
  %row = getelementptr inbounds i8, ptr @g, i32 3840
  call void @llvm.experimental.memset.pattern.p0.i16.i32(ptr %row, i16 1824, i32 80, i1 false)
  %dst = getelementptr inbounds i8, ptr @g, i32 160
  call void @llvm.memmove.p0.p0.i32(ptr %dst, ptr @g, i32 3840, i1 false)
  ret void
}
",
        &[&[]],
    );
    assert!(!after.contains("call void @llvm.experimental.memset.pattern"), "{after}");
    assert!(after.contains("call void @llvm.memmove"), "{after}");
}

/// A copy that reads the fill it covers needs it: the source is the blank row.
#[test]
fn a_memset_pattern_read_by_the_covering_memmove_stays() {
    let after = eliminated(
        "@g = global [2000 x i16] zeroinitializer
declare void @llvm.experimental.memset.pattern.p0.i16.i32(ptr, i16, i32, i1)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)

define void @f() {
b0:
  %row = getelementptr inbounds i8, ptr @g, i32 3840
  call void @llvm.experimental.memset.pattern.p0.i16.i32(ptr %row, i16 1824, i32 80, i1 false)
  %src = getelementptr inbounds i8, ptr @g, i32 3000
  call void @llvm.memmove.p0.p0.i32(ptr @g, ptr %src, i32 1000, i1 false)
  %dst = getelementptr inbounds i8, ptr @g, i32 160
  call void @llvm.memmove.p0.p0.i32(ptr %dst, ptr %row, i32 3840, i1 false)
  ret void
}
",
        &[&[]],
    );
    assert!(after.contains("call void @llvm.experimental.memset.pattern"), "{after}");
}

/// A chain of overwrites costs in proportion to its length.
#[test]
fn a_long_chain_of_overwrites_is_linear() {
    let chain = |n: usize| {
        let stores: String = (0..n).map(|k| format!("  store i16 {k}, ptr @g\n")).collect();
        format!(
            "@g = global i16 0\n\ndefine i16 @f(i16 %x) {{\nb0:\n{stores}  %r = load i16, ptr @g\n  ret i16 %r\n}}\n"
        )
    };
    let work = |n: usize| {
        let before = super::steps();
        let after = eliminated(&chain(n), &[&[1]]);
        assert_eq!(after.matches("store i16").count(), 1, "{after}");
        super::steps() - before
    };
    let (small, large) = (work(100), work(200));
    assert!(large as f64 <= 2.5 * small as f64, "{small} steps for 100 stores, {large} for 200");
}

/// Blocks are walked in LLVM's post-order, a branch's first target first.
/// Walked the other way, the memset arm killed the word store (through the
/// bytes the final store and the memset cover) before the first arm's byte
/// store had killed the dword store, which the read in the join then needs;
/// `opt` keeps it.
#[test]
fn the_first_target_of_a_branch_is_walked_first() {
    let after = eliminated(
        "@g = global [64 x i8] zeroinitializer\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  %c = icmp eq i16 %x, 0\n  br i1 %c, label %b1, label %b2\nb1:\n  store i32 3, ptr @g\n  %p1 = getelementptr i8, ptr @g, i16 1\n  store i8 1, ptr %p1\n  br label %b3\nb2:\n  %p2 = getelementptr i8, ptr @g, i16 1\n  call void @llvm.memset.p0.i16(ptr %p2, i8 1, i16 2, i1 false)\n  br label %b3\nb3:\n  store i8 0, ptr @g\n  %p3 = getelementptr i8, ptr @g, i16 2\n  store i16 1, ptr %p3\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n",
        &[&[0], &[1]],
    );
    assert!(after.contains("store i16 1, ptr @g"), "{after}");
}

/// A local whose address is returned, or stored into another returned local,
/// is read by the caller: its stores stay. All of `%view`'s and `%slot`'s went
/// (QB view descriptors), six BASIC programs read zeros.
#[test]
fn a_returned_local_keeps_its_stores() {
    let mut module = parsed(&format!(
        "{DOS}define ptr @f(i8 %x) addrspace(1) {{\nb0:\n  %slot = alloca i8\n  %view = alloca [4 x i8]\n  store i8 %x, ptr %slot\n  store i16 1, ptr %view\n  %p = getelementptr i8, ptr %view, i16 2\n  store ptr %slot, ptr %p\n  ret ptr %view\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(
        after.contains("store i8 %x") && after.contains("store i16 1") && after.contains("store ptr %slot"),
        "{after}"
    );
}

/// A store to a local whose address escaped, and then `lifetime.end`: nothing
/// reads it again and the end sweep spares it (it is visible to the callee's
/// holder), but the terminator kills it (`isMemTerminator`).
#[test]
fn a_store_before_lifetime_end_goes() {
    let mut module = parsed(&format!(
        "{DOS}declare void @keep(ptr)\ndeclare void @llvm.lifetime.end.p0(i64, ptr)\n\ndefine void @f(i32 %x) addrspace(1) {{\nb0:\n  %a = alloca i32\n  call void @keep(ptr %a)\n  store i32 %x, ptr %a\n  call void @llvm.lifetime.end.p0(i64 4, ptr %a)\n  ret void\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(!after.contains("store i32 %x"), "{after}");
}

/// `lifetime.end` on an alloca the function returns (QB's B$FMID shape) ends
/// nothing: the caller reads what was stored. The marker was wrong, and a DSE
/// that trusted it removed the stores (#2625).
#[test]
fn a_lifetime_end_on_a_returned_alloca_kills_nothing() {
    let mut module = parsed(&format!(
        "{DOS}declare void @llvm.lifetime.end.p0(i64, ptr)\n\ndefine ptr @f(i32 %x) addrspace(1) {{\nb0:\n  %a = alloca i32\n  store i32 %x, ptr %a\n  call void @llvm.lifetime.end.p0(i64 4, ptr %a)\n  ret ptr %a\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(after.contains("store i32 %x"), "{after}");
}

/// A store the next call overwrites before reading, by the `initializes` of
/// the callee's parameter, is dead (`getInitializesArgMemLoc`).
#[test]
fn a_store_before_a_call_that_initializes_it_goes() {
    let mut module = parsed(&format!(
        "{DOS}@g = global i32 0\n\ndeclare void @init(ptr initializes((0, 4)) %p) memory(argmem: write) nounwind\n\ndefine i32 @f(i32 %x) addrspace(1) {{\nb0:\n  store i32 %x, ptr @g\n  call void @init(ptr @g)\n  %r = load i32, ptr @g\n  ret i32 %r\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(!after.contains("store i32 %x"), "{after}");
}

/// A store over the head of a memset leaves its tail, which a load reads: the
/// memset went, and the load at byte 6 read an undefined byte.
#[test]
fn a_memset_whose_tail_is_read_stays() {
    let after = eliminated(
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine i16 @f(i16 %x) {\nb0:\n  %s = alloca [8 x i8]\n  call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 8, i1 false)\n  store i16 %x, ptr %s\n  %s6 = getelementptr inbounds i8, ptr %s, i16 6\n  %v = load i16, ptr %s6\n  %w = load i16, ptr %s\n  %r = add i16 %v, %w\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(after.contains("i8 0, i16 6"), "{after}");
}

/// A call that fills a buffer reads it first through another argument: the
/// memset before it stays. It went, and the callee read poison.
#[test]
fn a_call_reading_the_buffer_it_fills_keeps_the_earlier_fill() {
    let after = eliminated(
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n\ndefine void @fill(ptr %d, ptr %s) {\nb:\n  %v = load i16, ptr %s\n  store i16 %v, ptr %d\n  %e = getelementptr i8, ptr %d, i16 2\n  store i16 %v, ptr %e\n  ret void\n}\n\ndefine i16 @f(i16 %x) {\nb0:\n  %a = alloca [4 x i8]\n  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 4, i1 false)\n  call void @fill(ptr nocapture writeonly initializes((0, 4)) %a, ptr %a)\n  %v = load i16, ptr %a\n  ret i16 %v\n}\n",
        &[&[7]],
    );
    assert!(after.contains("llvm.memset.p0.i16(ptr %a"), "{after}");
}

/// Only a store, or a call that fills bytes, can be dead: a body with neither
/// built the memory graph and solved what escapes for nothing.
#[test]
fn a_body_with_no_store_asks_nothing() {
    use llrm_analysis::manager::Summaries;
    use llrm_mir::passes::PassManager;
    let mut module =
        parsed("declare i16 @g(i16)\n\ndefine i16 @f(i16 %x) {\nb0:\n  %y = call i16 @g(i16 %x)\n  ret i16 %y\n}\n");
    let mut manager = PassManager::default();
    manager.require::<Summaries>();
    manager.add(DeadStoreElimination);
    llrm_mir::passes::trace_recomputes(true);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let counts = llrm_mir::passes::recomputes();
    llrm_mir::passes::trace_recomputes(false);
    let asked: Vec<_> =
        counts.iter().filter(|(name, ..)| matches!(*name, "points-to" | "published" | "accesses")).collect();
    assert!(asked.is_empty(), "{asked:?}");
}

/// The cells of one array, each stored twice, cost in proportion to their
/// number (the old solver compared every cell with all of its object's, and
/// LLVM's walk is capped).
#[test]
fn the_stores_of_one_array_cost_in_proportion_to_their_number() {
    let work = |cells: usize| {
        let stores: String =
            (0..cells).map(|i| format!("  store i16 {i}, ptr getelementptr (i8, ptr @arr, i16 {})\n", 2 * i)).collect();
        let text = format!(
            "@arr = global [{} x i8] zeroinitializer\n\ndefine i16 @f() {{\nb0:\n{stores}{stores}  ret i16 0\n}}\n",
            2 * cells
        );
        let before = super::steps();
        let after = eliminated(&text, &[&[]]);
        let _ = after;
        super::steps() - before
    };
    let (small, large) = (work(100), work(400));
    assert!(large as f64 <= 5.0 * small as f64, "{small} steps for 100 cells, {large} for 400");
}

/// `getUnderlyingObject` of two pointers that strip to one value is one object,
/// whatever the value is (a loaded pointer here): a store inside the bytes a
/// later store covers is dead. The object was taken from annotated provenance,
/// which a loaded pointer has none of, so the small store stayed.
#[test]
fn a_store_inside_a_later_wider_one_through_a_loaded_pointer_goes() {
    let mut module = parsed(&format!(
        "{DOS}define void @f(ptr %pp) {{\nb0:\n  %p = load ptr, ptr %pp\n  %q = getelementptr i8, ptr %p, i16 2\n  store i16 1, ptr %q\n  store i32 2, ptr %p\n  ret void\n}}\n"
    ));
    let after = managed(&mut module, DeadStoreElimination);
    assert!(!after.contains("store i16 1"), "{after}");
}

/// DSE took the manager's annotated accesses, deriving every reference's
/// provenance for a body (3.7 M Ir in d_faces) to learn what a store writes.
/// It asks the unit lazily now: a run derives no annotations.
#[test]
fn a_run_derives_no_annotations() {
    let mut module = parsed(&format!(
        "{DOS}@g = global i16 0\n\ndefine i16 @f(i16 %x) {{\nb0:\n  store i16 1, ptr @g\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}}\n"
    ));
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(DeadStoreElimination);
    let (_, before) = llrm_mir::counts::analysed("annotated");
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let (_, after) = llrm_mir::counts::analysed("annotated");
    assert!(!crate::testing::printed(&module).contains("store i16 1"), "premise: the store goes");
    assert_eq!(after - before, 0, "DSE derived the annotations");
}

/// DSE forked the whole body (and cloned the context) to read it while the
/// removals waited: one fork, 1.74 M Ir, for each run over a body with a
/// store.
#[test]
fn a_run_forks_no_body() {
    use llrm_mir::counts::{Event, get};
    let before = get(Event::FunctionForks);
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("store i16 1"), "{after}");
    assert_eq!(get(Event::FunctionForks), before, "dse forked the body");
}

/// A pass that asks for MemorySSA, without editing.
struct AsksMssa;

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
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// DSE built its own MemorySSA (0.77 M Ir, 2 builds in d_faces) beside the
/// one the manager held: it reads the held graph now (a hit, not a build).
#[test]
fn the_graph_the_manager_holds_is_the_one_dse_reads() {
    let mut module = parsed(&format!(
        "{DOS}@g = global i16 0\n\ndefine i16 @f(i16 %x) {{\nb0:\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}}\n"
    ));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add_boxed(Box::new(AsksMssa));
    manager.add_boxed(Box::new(DeadStoreElimination));
    let before = llrm_mir::counts::analysed("mssa");
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    let after = llrm_mir::counts::analysed("mssa");
    assert_eq!((after.0 - before.0, after.1 - before.1), (1, 1), "DSE did not read the held graph (hits, misses)");
}

/// `cycle_of` scanned every loop's body for each pair of stores it was asked
/// about (0.84 M Ir in d_faces' `loop_independent`); the map made once must
/// name the smallest loop of each block, as the scan did.
#[test]
fn the_innermost_loop_of_a_block_is_the_smallest_that_holds_it() {
    let module = parsed(&format!(
        "{DOS}define i32 @f(i32 %n) {{\nb0:\n  br label %o\no:\n  %i = phi i32 [0, %b0], [%j, %olatch]\n  br label %n1\nn1:\n  %k = phi i32 [0, %o], [%k2, %n1]\n  %k2 = add i32 %k, 1\n  %c = icmp slt i32 %k2, %n\n  br i1 %c, label %n1, label %olatch\nolatch:\n  %j = add i32 %i, 1\n  %d = icmp slt i32 %j, %n\n  br i1 %d, label %o, label %x\nx:\n  ret i32 %j\n}}\n"
    ));
    let (_, _, function) = module.functions().next().expect("a function");
    let shape = llrm_analysis::cfg::Shape::of(function);
    let found = super::innermost_headers(&shape.loops);
    for &block in function.layout() {
        let scan = shape
            .loops
            .iter()
            .filter(|one| one.body.contains(&llrm_analysis::cfg::id(block)))
            .min_by_key(|one| one.body.len())
            .map(|one| one.header);
        assert_eq!(found.get(&block).copied(), scan, "{block:?}");
    }
    assert_eq!(shape.loops.len(), 2);
}

/// `may_write` and `may_read` derived a load's or store's whole list to learn
/// it is not empty (`loc_for_write` 0.78 M Ir in d_faces, 609 lists): a body of
/// loads and stores derives none.
#[test]
fn asking_whether_a_load_or_store_writes_derives_no_list() {
    let before = llrm_analysis::memoryssa::lazy_lists();
    let after = eliminated(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  store i16 1, ptr @g\n  store i16 %x, ptr @g\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        &[&[7]],
    );
    assert!(!after.contains("store i16 1"), "{after}");
    assert_eq!(llrm_analysis::memoryssa::lazy_lists(), before, "lists derived for loads and stores");
}

/// A store's place was worked out again at every ask (603 times for about a
/// hundred stores on d_faces). A chain of n stores works out at most one place
/// for each of its instructions.
#[test]
fn a_place_is_worked_out_once_for_each_instruction() {
    let stores: String = (0..50).map(|k| format!("  store i16 {k}, ptr @g\n")).collect();
    let body = format!(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {{\nb0:\n{stores}  %r = load i16, ptr @g\n  ret i16 %r\n}}\n"
    );
    let before = super::locations();
    eliminated(&body, &[&[1]]);
    let made = super::locations() - before;
    assert!(made <= 52, "{made} places worked out for 52 instructions");
}

/// `n` locals, each stored twice (the first store is dead) and read once.
fn locals(n: usize) -> String {
    let mut text = String::from("define i32 @f() {\nb0:\n");
    for k in 0..n {
        text += &format!("  %p{k} = alloca i32\n  store i32 1, ptr %p{k}\n  store i32 2, ptr %p{k}\n");
    }
    text += "  %s0 = add i32 0, 0\n";
    for k in 0..n {
        text += &format!("  %v{k} = load i32, ptr %p{k}\n  %s{} = add i32 %s{k}, %v{k}\n", k + 1);
    }
    text + &format!("  ret i32 %s{n}\n}}\n")
}

/// `object_of` walked an alloca's uses at every access asked of it, since the
/// pass did not hand it the exposed frames: matmul -O2 made 27,548 walks and
/// 1.9 M tree inserts in a third of the pass. The walks are the frames', not
/// the accesses'.
#[test]
fn the_uses_of_a_local_are_walked_once_however_many_accesses_ask() {
    let scans = |n: usize| {
        let mut module = parsed(&locals(n));
        let before = llrm_analysis::frameescape::scans();
        managed(&mut module, DeadStoreElimination);
        llrm_analysis::frameescape::scans() - before
    };
    let (small, large) = (scans(8), scans(64));
    assert!(large <= 64 + 8 && small <= 8 + 8, "8 locals: {small} walks, 64 locals: {large}");
}
