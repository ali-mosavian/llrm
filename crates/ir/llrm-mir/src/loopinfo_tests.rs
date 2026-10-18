use crate::domtree::DomTree;
use crate::loopinfo::{LoopInfo, print_loops};
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
    let info = LoopInfo::analyze(function, || DomTree::of(function));
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
    let info = LoopInfo::analyze(function, || DomTree::of(function));
    let one = info.get_loop_for(block(function, "h")).expect("loop");
    assert_eq!(one.get_loop_latch(function), None, "two latches");
    assert_eq!(names(function, one.get_loop_latches(function)), ["a", "b"]);
    assert_eq!(one.get_num_back_edges(function), 2);
    assert_eq!(one.get_loop_predecessor(function), Some(block(function, "entry")));
    assert_eq!(one.get_loop_preheader(function), Some(block(function, "entry")));

    let function = find("self");
    let info = LoopInfo::analyze(function, || DomTree::of(function));
    let one = info.get_loop_for(block(function, "l")).expect("loop");
    assert_eq!(one.get_loop_latch(function), Some(block(function, "l")));
    assert_eq!(info.get_unique_latch_exit_block(function, one), Some(block(function, "exit")));
}

#[test]
fn test_nesting_preorder_depth_and_common_loop() {
    let module = load("shapes.ll");
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("nest")).unwrap();
    let info = LoopInfo::analyze(function, || DomTree::of(function));
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

/// `Loop::isLoopInvariant(const Value *)`: an argument, a constant and an
/// instruction before the loop are invariant; the loop's own are not.
#[test]
fn test_loop_invariance_of_a_value_is_where_its_instruction_is() {
    let module = parse::module(
        "define i32 @f(i32 %n) {
entry:
  %pre = add i32 %n, 1
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %i2 = add i32 %i, %pre
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret i32 %i2
}
",
    )
    .expect("a module");
    let function = module.functions().next().expect("a function").2;
    let info = LoopInfo::analyze(function, || DomTree::of(function));
    let one = info.get_loop_for(block(function, "h")).expect("loop");
    let results: Vec<_> = function.walk().filter_map(|(_, inst)| function.instruction(inst).result).collect();
    let [pre, i, i2, c] = results[..] else { panic!("four results") };
    use crate::module::Operand::Value;
    assert!(one.is_loop_invariant(function, Value(function.parameters()[0])), "an argument");
    assert!(one.is_loop_invariant(function, Value(pre)), "defined before the loop");
    assert!(!one.is_loop_invariant(function, Value(i)), "a phi of the loop");
    assert!(!one.is_loop_invariant(function, Value(i2)));
    assert!(!one.is_loop_invariant(function, Value(c)));
}
