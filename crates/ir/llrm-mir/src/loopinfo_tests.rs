use crate::dominators::DominatorTree;
use crate::loopinfo::{LoopBlocksDfs, LoopBlocksRpo, LoopInfo, print_loops};
use crate::module::{BlockId, Function, Module};
use crate::parse;

fn load(file: &str) -> Module {
    let path = format!("{}/tests/loops/{file}", env!("CARGO_MANIFEST_DIR"));
    parse::module(&std::fs::read_to_string(&path).expect("fixture")).unwrap_or_else(|error| panic!("{error}"))
}

fn block(
    function: &Function,
    name: &str,
) -> BlockId {
    *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some(name)).expect("block")
}

fn names(
    function: &Function,
    mut blocks: Vec<BlockId>,
) -> Vec<String> {
    blocks.sort();
    blocks.iter().map(|&one| function.block(one).name.clone().expect("named")).collect()
}

/// `opt -passes='print<loops>'` (pinned LLVM 24.0.0git) wrote the `.expected`
/// files. Before the translation the forest found the same blocks but not
/// LLVM's block order, the reduction of an irreducible cycle to its natural
/// loop, or the `<latch>` and `<exiting>` marks.
#[test]
fn test_print_loops_equals_the_pinned_opt_on_irreducible_and_odd_shapes() {
    for file in ["irreducible", "shapes"] {
        let want = std::fs::read_to_string(format!("{}/tests/loops/{file}.expected", env!("CARGO_MANIFEST_DIR")))
            .expect("expected");
        // Premise: the oracle's text holds a loop (the irreducible cycle's
        // outer loop; a nest in `shapes`); an empty tree would equal
        // anything empty.
        assert!(want.contains("Loop at depth 1"), "{file}");
        assert!(file != "shapes" || want.contains("    Loop at depth 2"), "{file}");
        assert_eq!(print_loops(&load(&format!("{file}.ll"))), want, "{file}");
    }
}

/// A branch naming one block twice is two edges: `getExitBlocks` repeats it,
/// `getExitBlock` is then null, `getUniqueExitBlock` is the block.
#[test]
fn test_exit_blocks_keep_repeats_and_unique_exit_does_not() {
    let module = load("shapes.ll");
    let (_, _, function) =
        module.functions().find(|(_, global, _)| global.name.as_deref() == Some("switch_exits")).unwrap();
    let info = LoopInfo::analyze(function, || DominatorTree::new(function));
    let one = info.get_loop_for(block(function, "h")).expect("loop");
    let exit = block(function, "exit");
    assert_eq!(one.get_exit_blocks(function), vec![exit, exit]);
    assert_eq!(one.get_exit_block(function), None);
    assert_eq!(one.get_unique_exit_block(function), Some(exit));
    assert_eq!(one.get_unique_exit_blocks(function), vec![exit]);
    assert!(one.has_dedicated_exits(function));
    assert_eq!(one.get_exiting_blocks(function), vec![block(function, "h")]);
}

#[test]
fn test_latch_predecessor_and_preheader() {
    let module = load("shapes.ll");
    let find = |name: &str| module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).unwrap().2;
    let function = find("two_latches");
    let info = LoopInfo::analyze(function, || DominatorTree::new(function));
    let one = info.get_loop_for(block(function, "h")).expect("loop");
    assert_eq!(one.get_loop_latch(function), None, "two latches");
    assert_eq!(names(function, one.get_loop_latches(function)), ["a", "b"]);
    assert_eq!(one.get_num_back_edges(function), 2);
    assert_eq!(one.get_loop_predecessor(function), Some(block(function, "entry")));
    assert_eq!(one.get_loop_preheader(function), Some(block(function, "entry")));

    let function = find("self");
    let info = LoopInfo::analyze(function, || DominatorTree::new(function));
    let one = info.get_loop_for(block(function, "l")).expect("loop");
    assert_eq!(one.get_loop_latch(function), Some(block(function, "l")));
    assert_eq!(info.get_unique_latch_exit_block(function, one), Some(block(function, "exit")));
}

#[test]
fn test_nesting_preorder_depth_and_common_loop() {
    let module = load("shapes.ll");
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("nest")).unwrap();
    let info = LoopInfo::analyze(function, || DominatorTree::new(function));
    let headers: Vec<_> =
        info.get_loops_in_preorder().iter().map(|one| function.block(one.get_header()).name.clone().unwrap()).collect();
    assert_eq!(headers, ["o", "i", "j", "k"], "program order, outer first");
    let top: Vec<_> =
        info.top_level_loops().map(|one| function.block(one.get_header()).name.clone().unwrap()).collect();
    assert_eq!(top, ["j", "o"], "top-level loops are in reverse program order");
    assert_eq!(info.get_loop_depth(block(function, "i")), 2);
    assert_eq!(info.get_loop_depth(block(function, "exit")), 0);
    assert!(info.is_loop_header(block(function, "k")));
    assert!(!info.is_loop_header(block(function, "ol")));
    let (i, k) = (info.get_loop_for(block(function, "i")), info.get_loop_for(block(function, "k")));
    assert_eq!(info.get_smallest_common_loop(i, k), None);
    assert_eq!(
        info.get_smallest_common_loop(i, info.get_loop_for(block(function, "ol"))),
        info.get_loop_for(block(function, "o"))
    );
}

/// 479131558 kept dominance and loops per CFG shape so a pass that leaves the
/// CFG alone finds them. The same is asked of `LoopInfo` through the manager:
/// three asks, and an invalidation that preserves everything, are one analysis,
/// and a CFG edit finds them again. A `LoopInfo` found afresh each time would
/// count one per ask.
#[test]
fn test_loops_are_found_once_while_the_cfg_is_kept() {
    use std::rc::Rc;

    use crate::counts::{Event, get};
    use crate::passes::{Analyses, Loops, Outer, PreservedAnalyses};
    let module = load("shapes.ll");
    let mut function = module
        .functions()
        .find(|(_, global, _)| global.name.as_deref() == Some("nest"))
        .map(|(_, _, function)| function.clone())
        .unwrap();
    let function = &mut function;
    let layout = crate::datalayout::DataLayout::default();
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    let before = get(Event::Forests);
    for _ in 0..3 {
        analyses.get::<Loops>(&module.context, &layout, function);
    }
    analyses.invalidate(function, &PreservedAnalyses::all());
    analyses.get::<Loops>(&module.context, &layout, function);
    assert_eq!(get(Event::Forests) - before, 1, "premise and keep: found once");
    // The inner loop `i` no longer loops: its branch goes to `ol` twice.
    let branch = function.terminator(block(function, "i")).unwrap();
    let target = function.instruction(branch).operands[2];
    function.set_operand(branch, 1, target);
    analyses.invalidate(function, &PreservedAnalyses::none());
    let found = analyses.get::<Loops>(&module.context, &layout, function);
    assert_eq!(get(Event::Forests) - before, 2, "control: the CFG changed, found again");
    assert!(!found.is_loop_header(block(function, "i")));
}

fn value_named(
    function: &Function,
    name: &str,
) -> crate::module::ValueId {
    function
        .parameters
        .iter()
        .copied()
        .chain(function.walk().filter_map(|(_, inst)| function.instruction(inst).result))
        .find(|&one| function.value(one).name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("value {name}"))
}

fn find<'a>(
    module: &'a Module,
    name: &str,
) -> &'a Function {
    module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).expect("a function").2
}

/// `isLCSSAForm`: a use of a loop-defined value outside the loop, not in a
/// phi of the exit, makes the loop not LCSSA; the phi closes it.
#[test]
fn test_a_value_used_outside_the_loop_without_a_phi_is_not_lcssa() {
    let module = load("queries.ll");
    for (name, closed) in [("leaks", false), ("closed", true)] {
        let function = find(&module, name);
        let tree = DominatorTree::new(function);
        let info = LoopInfo::analyze(function, || &tree);
        let one = info.get_loop_for(block(function, "h")).expect("loop");
        assert_eq!(one.is_lcssa_form(function, &tree), closed, "{name}");
        assert_eq!(one.is_recursively_lcssa_form(function, &tree, &info), closed, "{name}");
    }
}

#[test]
fn test_loop_invariance_is_a_definition_outside_the_loop() {
    let module = load("queries.ll");
    let function = find(&module, "invariance");
    let info = LoopInfo::analyze(function, || DominatorTree::new(function));
    let one = info.get_loop_for(block(function, "h")).expect("loop");
    let value = |name: &str| crate::module::Operand::Value(value_named(function, name));
    assert!(one.is_loop_invariant(function, value("k")), "defined in the entry");
    assert!(one.is_loop_invariant(function, value("n")), "an argument");
    assert!(!one.is_loop_invariant(function, value("i")), "a header phi");
    let inst = |name: &str| match function.value(value_named(function, name)).def {
        crate::module::ValueDef::Instruction(inst) => inst,
        crate::module::ValueDef::Argument(_) => unreachable!(),
    };
    assert!(one.has_loop_invariant_operands(function, inst("m")), "k + n");
    assert!(!one.has_loop_invariant_operands(function, inst("j")), "i + k");
}

#[test]
fn test_loop_simplify_form_and_the_incoming_and_back_edges() {
    let module = load("shapes.ll");
    let (selfloop, two) = (find(&module, "self"), find(&module, "two_latches"));
    let info = LoopInfo::analyze(selfloop, || DominatorTree::new(selfloop));
    let one = info.get_loop_for(block(selfloop, "l")).expect("loop");
    assert!(one.is_loop_simplify_form(selfloop));
    assert_eq!(one.get_incoming_and_back_edge(selfloop), Some((block(selfloop, "entry"), block(selfloop, "l"))));
    let info = LoopInfo::analyze(two, || DominatorTree::new(two));
    let one = info.get_loop_for(block(two, "h")).expect("loop");
    assert!(!one.is_loop_simplify_form(two), "two latches");
    assert_eq!(one.get_incoming_and_back_edge(two), None, "three edges into the header");
}

/// `LoopBlocksRPO`, by hand: from `h` the search takes `a` before `b`; `m`
/// finishes first, then `a`, `b`, `h`: postorder m a b h, reverse h b a m.
#[test]
fn test_loop_blocks_rpo_is_the_reverse_of_the_search_postorder() {
    let module = load("queries.ll");
    let function = find(&module, "diamond");
    let info = LoopInfo::analyze(function, || DominatorTree::new(function));
    let one = info.get_loop_for(block(function, "h")).expect("loop");
    let mut dfs = LoopBlocksDfs::new(one);
    assert!(!dfs.has_preorder(block(function, "h")));
    dfs.perform(function);
    let [h, a, b, m] = ["h", "a", "b", "m"].map(|name| block(function, name));
    assert!(dfs.is_complete());
    assert_eq!(dfs.postorder(), [m, a, b, h]);
    assert_eq!(dfs.rpo().collect::<Vec<_>>(), [h, b, a, m]);
    assert_eq!((dfs.get_postorder(m), dfs.get_rpo(m), dfs.get_rpo(h)), (1, 4, 1));
    assert!(!dfs.has_preorder(block(function, "exit")), "outside the loop");
    let mut rpo = LoopBlocksRpo::new(one);
    rpo.perform(function);
    assert_eq!(rpo.iter().collect::<Vec<_>>(), [h, b, a, m]);
}
