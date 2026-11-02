//! The parts of `LoopIdiomRecognize` that read no add recurrence:
//! `getMemSetPatternValue` and the stores `isLegalStore` refuses before it
//! asks ScalarEvolution. The accepted stores wait for `createSCEV` (S3): a
//! `getSCEV` that is an unknown matches no add recurrence.

use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId, Module};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::AssumptionCache;

use super::{LegalStoreKind, Library, LoopIdiomRecognize, Options, get_mem_set_pattern_value};

const LIBRARY: Library = Library { memset: true, memset_pattern16: false, memcpy: true };

/// Every store of `@f`'s loop header block `h`, as `is_legal_store` sees it.
fn kinds(
    text: &str,
    options: Options,
    library: Library,
) -> Vec<LegalStoreKind> {
    let module: Module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("a function");
    let layout = DataLayout::default();
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
    let header: BlockId =
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let cur_loop = loops.get_loop_for(header).expect("a loop");
    let mut se = ScalarEvolution::new();
    let idiom = LoopIdiomRecognize::new(options, library);
    function
        .iter_block(header)
        .filter(|&inst: &InstId| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
        .map(|inst| idiom.is_legal_store(&cx, &mut se, cur_loop, inst))
        .collect()
}

const STORES: &str = "define void @f(ptr %p, i32 %n) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %a = getelementptr i8, ptr %p, i32 %i
  store volatile i8 0, ptr %a
  store i8 0, ptr %a, !nontemporal !0
  store i8 0, ptr %a
  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret void
}

!0 = !{i32 1}
";

/// A volatile store and a nontemporal one are not memset candidates; the plain
/// byte store of zero at `%p + i` is (it was `None` while `getSCEV` was an
/// unknown for every address).
#[test]
fn test_a_store_that_is_volatile_or_nontemporal_is_not_legal() {
    assert_eq!(
        kinds(STORES, Options::default(), LIBRARY),
        vec![LegalStoreKind::None, LegalStoreKind::None, LegalStoreKind::Memset]
    );
}

#[test]
fn test_the_pattern_value_is_a_power_of_two_sized_int_float_or_pointer_constant() {
    let module = llrm_mir::parse::module(
        "@g = global i32 0
define void @f(ptr %p, i32 %x) {
  store i32 7, ptr %p
  store i32 %x, ptr %p
  store i24 7, ptr %p
  store i64 7, ptr %p
  store i128 7, ptr %p
  store double 1.0, ptr %p
  store x86_fp80 1.0, ptr %p
  store ptr @g, ptr %p
  store ptr getelementptr (i8, ptr @g, i64 1), ptr %p
  store [2 x i32] zeroinitializer, ptr %p
  store i1 true, ptr %p
  ret void
}
",
    )
    .expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("a function");
    let layout = DataLayout::default();
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
    let found: Vec<bool> = function
        .walk()
        .filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
        .map(|(_, inst)| get_mem_set_pattern_value(&cx, function.instruction(inst).operands[0]).is_some())
        .collect();
    // i32 7 yes; an argument no; i24 not a power of two; i64, i128 yes; double
    // yes; x86_fp80 is 80 bits, not a power of two; a
    // global's address yes; a constant expression no; an array is not an int,
    // float or pointer; i1 is not a whole byte.
    assert_eq!(found, [true, false, false, true, true, true, false, true, false, false, false]);
}

/// `isSameByteValueStore` of each store of `%h` against the byte of the first.
fn same_byte(text: &str) -> Vec<bool> {
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("a function");
    let layout = DataLayout::default();
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
    let header: BlockId =
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let cur_loop = loops.get_loop_for(header).expect("a loop");
    let stores: Vec<InstId> = function
        .iter_block(header)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
        .collect();
    let splat = llrm_mir::valuetracking::bytewise::is_bytewise_value(
        &cx.query(Some(stores[0])),
        function.instruction(stores[0]).operands[0],
    )
    .expect("a byte");
    stores.iter().map(|&inst| super::is_same_byte_value_store(&cx, cur_loop, inst, splat)).collect()
}

#[test]
fn test_a_store_is_the_same_byte_only_if_simple_invariant_and_equal() {
    let text = "define void @f(ptr %p, i8 %v, i32 %n) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %a = getelementptr i8, ptr %p, i32 %i
  %t = trunc i32 %i to i8
  store i32 -1, ptr %a
  store i8 -1, ptr %a
  store i16 -1, ptr %a
  store i32 1, ptr %a
  store volatile i8 -1, ptr %a
  store i8 %v, ptr %a
  store i8 %t, ptr %a
  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret void
}
";
    // -1 as i32, i8 and i16 is the byte 0xff; i32 1 has none; a volatile store
    // is not simple; an i8 argument is its own byte, not 0xff; an i8 made in
    // the loop is not invariant.
    assert_eq!(same_byte(text), [true, true, true, false, false, false, false]);
}

/// `runOnLoop` on the loop headed by `%h` of the module's one function, named
/// `name`: what it answers before it reads an add recurrence.
fn run_on_loop(
    text: &str,
    name: &'static str,
) -> Option<bool> {
    use llrm_analysis::scev::expander::Env;

    use super::process::{AaProvider, Run};

    struct NoAa;
    impl AaProvider for NoAa {
        fn with(
            &self,
            _: &llrm_mir::context::Context,
            _: &llrm_mir::module::Function,
            _: &mut dyn FnMut(&mut dyn llrm_analysis::aa::AAResults) -> bool,
        ) -> bool {
            unreachable!("no alias query before a store is found")
        }
    }

    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, original) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("a function");
    let mut function = original.clone();
    let mut context = module.context.clone();
    let layout = DataLayout::default();
    let tree = DomTree::of(&function);
    let loops = LoopInfo::analyze(&function, || &tree);
    let assumptions = AssumptionCache::new(&module, &function);
    let env = Env {
        globals: &module.globals,
        metadata: &module.metadata,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let header: BlockId =
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let cur_loop = loops.get_loop_for(header).expect("a loop");
    let mut se = ScalarEvolution::new();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let mut run = Run {
        bytes: None,
        clocks: None,
        speed: None,
        name,
        env: &env,
        context: &mut context,
        function: &mut function,
        se: &mut se,
        aa: &NoAa,
        declared: &mut declared,
        cur_loop,
        tti: None,
    };
    LoopIdiomRecognize::new(Options::default(), LIBRARY).run_on_loop(&mut run)
}

const FILL: &str = "define void @f(ptr %p, i32 %n) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %a = getelementptr i8, ptr %p, i32 %i
  store i8 0, ptr %a
  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret void
}
";

/// A function named like the idiom it would be turned into is left alone
/// (`memset` calling itself through the call the pass would make).
#[test]
fn test_a_function_named_memset_is_left_alone() {
    assert_eq!(run_on_loop(FILL, "memset"), Some(false));
    assert_eq!(run_on_loop(FILL, "strlen"), Some(false));
}

const UNCOUNTED: &str = "define void @f(ptr %p, ptr %q) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %a = getelementptr i8, ptr %p, i32 %i
  store i8 0, ptr %a
  %i2 = add i32 %i, 1
  %l = load i32, ptr %q
  %c = icmp ne i32 %l, 0
  br i1 %c, label %h, label %exit

exit:
  ret void
}
";

/// Without a count the countable half does not decide and
/// `runOnNoncountableLoop` runs (stage L4, the string length so far): the exit
/// tests a load, but the module has no `strlen`, so nothing changes.
#[test]
fn test_a_loop_without_an_invariant_count_is_left_to_the_noncountable_half() {
    assert_eq!(run_on_loop(UNCOUNTED, "f"), Some(false));
}

/// `text` through `LoopIdiom` under the pass manager, printed.
fn pass_output(text: &str) -> String {
    let mut module = crate::testing::parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(super::pass::LoopIdiom::new());
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
    llrm_mir::print::module(&module)
}

/// The pass, joined at `loopidiom@737`, turns the byte fill into a memset;
/// before the join nothing ran `LoopIdiomRecognize` on a function.
#[test]
fn test_the_pass_turns_a_byte_fill_loop_into_a_memset() {
    let after = pass_output(FILL);
    assert!(after.contains("%0 = call i32 @llvm.smax.i32(i32 %n, i32 1)"), "{after}");
    assert!(after.contains("%1 = zext nneg i32 %0 to i64"), "{after}");
    assert!(after.contains("call void @llvm.memset.p0.i64(ptr align 1 %p, i8 0, i64 %1, i1 false)"), "{after}");
}

/// A function with `"no-builtins"` has every library function disabled in the
/// pin's `TargetLibraryInfo`: x_radix (Open Watcom, `-fno-builtin`) had its
/// zeroing loop made a memset, which the pin leaves a loop, and the unroll
/// after it then sized the enclosing loop differently (90 to 179 bytes).
#[test]
fn test_a_no_builtins_function_gets_no_memset() {
    let text = FILL.replace("define void @f(ptr %p, i32 %n) {", "define void @f(ptr %p, i32 %n) \"no-builtins\" {");
    let after = pass_output(&text);
    assert!(!after.contains("llvm.memset"), "{after}");
    assert!(pass_output(FILL).contains("llvm.memset"));
}

/// The trip counts `runs` loop-idiom passes in a row work out over `text`.
fn idiom_trip_counts(
    text: &str,
    runs: usize,
) -> usize {
    let mut module = crate::testing::parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    for _ in 0..runs {
        manager.add(super::pass::LoopIdiom::new());
    }
    let before = llrm_analysis::scev::trip::backedge_taken_computations();
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    llrm_analysis::scev::trip::backedge_taken_computations() - before
}

/// Every run started a `ScalarEvolution` of its own, so a loop it left alone
/// had its trip count worked out again by the next pass (53 of d_faces -O2's
/// 226 counts were such repeats). A run that changes nothing hands its
/// expressions on.
#[test]
fn test_a_loop_the_pass_leaves_alone_is_not_counted_again_by_the_next_run() {
    let text = FILL.replace("  store i8 0, ptr %a\n", "  store i8 0, ptr %a\n  %l = load i8, ptr %a\n");
    let (once, twice) = (idiom_trip_counts(&text, 1), idiom_trip_counts(&text, 2));
    assert!(once > 0, "the pass asks for the count: {once}");
    assert_eq!(twice, once, "one run {once} counts, two runs {twice}");
}

/// The load/store/default dispatch of `getModRefInfo(Instruction)`: BasicAA
/// answered ModRef for every instruction, so no loop with a phi could pass
/// `mayLoopAccessLocation` and the fill above stayed a loop.
#[test]
fn test_a_loop_that_reads_what_it_fills_is_left_alone() {
    let text = FILL.replace("  store i8 0, ptr %a\n", "  store i8 0, ptr %a\n  %l = load i8, ptr %a\n");
    let after = pass_output(&text);
    assert!(!after.contains("llvm.memset"), "{after}");
}

/// An in-place copy of `@arr` by `load_at` to `store_at` (offsets from the
/// counter, in elements), `step` per trip.
fn shift_loop(
    load_at: i32,
    store_at: i32,
    down: bool,
) -> String {
    let (start, step, test) = if down { (99, -1, "icmp sgt i16 %next, -1") } else { (0, 1, "icmp slt i16 %next, 100") };
    format!(
        "@arr = global [160 x i16] zeroinitializer

define void @f() {{
entry:
  br label %h

h:
  %i = phi i16 [ {start}, %entry ], [ %next, %h ]
  %li = add i16 %i, {load_at}
  %si = add i16 %i, {store_at}
  %from = getelementptr inbounds i16, ptr @arr, i16 %li
  %to = getelementptr inbounds i16, ptr @arr, i16 %si
  %v = load i16, ptr %from
  store i16 %v, ptr %to
  %next = add i16 %i, {step}
  %c = {test}
  br i1 %c, label %h, label %exit

exit:
  ret void
}}
"
    )
}

/// `a[i + 1] = a[i]` going up reads what it wrote: it spreads `a[0]`, which no
/// memmove does. The check that the store is not ahead of the load had its
/// sense inverted (`loadAndStoreMayFormMemmove` returned what LLVM's `return
/// false` condition said), so the loop became a memmove and `llrm-c` stopped
/// with "a memmove whose direction no pass proved" (tests/run/c/idioms.c, once
/// LoopRotate was on).
#[test]
fn test_a_loop_that_reads_what_it_wrote_is_not_a_memmove() {
    let after = pass_output(&shift_loop(0, 1, false));
    assert!(!after.contains("llvm.memmove") && !after.contains("llvm.memcpy"), "{after}");
    // The same going down: `a[i] = a[i + 1]` from the top.
    let after = pass_output(&shift_loop(1, 0, true));
    assert!(!after.contains("llvm.memmove") && !after.contains("llvm.memcpy"), "{after}");
}

/// A shift whose loads run ahead of its stores is a memmove.
#[test]
fn test_a_loop_that_shifts_a_buffer_the_safe_way_is_a_memmove() {
    let after = pass_output(&shift_loop(1, 0, false));
    assert!(after.contains("llvm.memmove"), "{after}");
    // llrm's code generator lowers a memmove only with its direction stated:
    // `llrm-c` stopped with "a memmove whose direction no pass proved".
    assert!(after.contains("!llrm.forward"), "{after}");
    let after = pass_output(&shift_loop(0, 1, true));
    assert!(after.contains("llvm.memmove"), "{after}");
    assert!(after.contains("!llrm.backward"), "{after}");
}

/// A copy loop over `__huge` arrays became a memmove, which the backend lowers
/// as `rep movs`: it does not carry into the selector past 64K, and
/// tests/run/huge/copyw printed 732590080 for 427417504 once LoopRotate was on.
#[test]
fn test_a_copy_of_a_huge_pointer_is_not_a_memcpy() {
    let text = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-f80:16-n8:16:32\"

@a = addrspace(3) global [40000 x i16] zeroinitializer
@b = addrspace(3) global [40000 x i16] zeroinitializer

define void @f() {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %next, %h ]
  %from = getelementptr inbounds i16, ptr addrspace(3) @a, i32 %i
  %to = getelementptr inbounds i16, ptr addrspace(3) @b, i32 %i
  %v = load i16, ptr addrspace(3) %from
  store i16 %v, ptr addrspace(3) %to
  %next = add i32 %i, 1
  %c = icmp ult i32 %next, 40000
  br i1 %c, label %h, label %exit

exit:
  ret void
}
";
    let after = pass_output(text);
    assert!(!after.contains("llvm.memcpy") && !after.contains("llvm.memmove"), "{after}");
}
