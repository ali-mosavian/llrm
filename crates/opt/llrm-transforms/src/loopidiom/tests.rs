//! The parts of `LoopIdiomRecognize` that read no add recurrence:
//! `getMemSetPatternValue` and the stores `isLegalStore` refuses before it
//! asks ScalarEvolution. The accepted stores wait for `createSCEV` (S3): a
//! `getSCEV` that is an unknown matches no add recurrence.

use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dominators::DominatorTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId, Module, Operand};
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
    let tree = DominatorTree::new(function);
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
        .block(header)
        .instructions()
        .iter()
        .copied()
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

/// A volatile store, a nontemporal one, and a plain one whose address is not
/// an add recurrence (before `createSCEV`): none is a memset candidate.
#[test]
fn test_a_store_that_is_volatile_nontemporal_or_not_strided_is_not_legal() {
    assert_eq!(kinds(STORES, Options::default(), LIBRARY), vec![LegalStoreKind::None; 3]);
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
    let tree = DominatorTree::new(function);
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
    let tree = DominatorTree::new(function);
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
        .block(header)
        .instructions()
        .iter()
        .copied()
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
    let tree = DominatorTree::new(&function);
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
        name,
        env: &env,
        context: &mut context,
        function: &mut function,
        se: &mut se,
        aa: &NoAa,
        declared: &mut declared,
        cur_loop,
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

/// Without a count the pass does not decide: `runOnNoncountableLoop` is the
/// C++'s next step (stage L4).
#[test]
fn test_a_loop_without_an_invariant_count_is_left_to_the_noncountable_half() {
    assert_eq!(run_on_loop(FILL, "f"), None);
}
