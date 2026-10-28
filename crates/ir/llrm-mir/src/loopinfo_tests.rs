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

/// Splices a new block `new` between `from` and its successor `to`: `from`'s
/// edge to `to` goes to `new`, which jumps to `to`.
fn splice_block(
    module: &mut Module,
    function: &str,
    from: &str,
    to: &str,
    new: &str,
) -> BlockId {
    use crate::module::Operand;
    use crate::opcode::{Flags, Opcode};
    let (_, f) = module.function_mut(function).expect("function");
    let (from, to) = (block(f, from), block(f, to));
    let created = f.create_block(Some(new));
    f.insert_block_before(created, to).expect("a new block");
    let void = f.instruction(f.terminator(from).expect("terminated")).ty;
    let jump = f.create_instruction(Opcode::Br, void, vec![Operand::Block(to)], Flags::default(), None);
    f.insert(jump, crate::edit::Position::End(created)).expect("placed");
    let term = f.terminator(from).expect("terminated");
    let operands = f
        .instruction(term)
        .operands
        .iter()
        .map(|&one| if one == Operand::Block(to) { Operand::Block(created) } else { one })
        .collect();
    f.set_operands(term, operands);
    created
}

/// A block added to a loop by `add_basic_block_to_loop` is in it and in every
/// loop around it, and the loops equal those of an analysis made afresh. The
/// loops after it in the block layout move, so their blocks must stay theirs.
/// The first version of the pass rebuilt the whole `LoopInfo` instead.
#[test]
fn test_a_block_added_to_a_loop_is_in_it_and_in_every_loop_around_it() {
    for (from, to, new, owner) in [("i", "i", "n_inner", "i"), ("ol", "o", "n_outer", "o"), ("jl", "j", "n_back", "j")]
    {
        let mut module = load("shapes.ll");
        let mut info = {
            let (_, _, function) =
                module.functions().find(|(_, global, _)| global.name.as_deref() == Some("nest")).unwrap();
            LoopInfo::analyze(function, || DomTree::of(function))
        };
        let created = splice_block(&mut module, "nest", from, to, new);
        let (_, _, function) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some("nest")).unwrap();
        let fresh = LoopInfo::analyze(function, || DomTree::of(function));
        assert!(fresh.get_loop_for(created).is_some(), "premise: {new} is in a loop");
        let owner_loop = info.get_loop_for(block(function, owner)).expect("the owner is in a loop").id();
        info.add_basic_block_to_loop(owner_loop, created);
        let label = |one: BlockId| function.block(one).name.clone().unwrap_or_default();
        assert!(
            info.equivalent(&fresh, function),
            "{new}:\n{}fresh:\n{}",
            info.print(function, "nest", &label),
            fresh.print(function, "nest", &label)
        );
        assert_eq!(
            info.get_loop_for(created).map(|one| one.get_header()),
            fresh.get_loop_for(created).map(|one| one.get_header()),
            "{new}"
        );
    }
}

fn only_loop<R>(
    text: &str,
    f: impl FnOnce(&crate::context::Context, &Function, crate::loopinfo::Loop<'_>) -> R,
) -> R {
    let module = parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    let function = module
        .functions()
        .find(|(_, _, function)| !function.is_declaration())
        .map(|(_, _, function)| function)
        .expect("a body");
    let info = LoopInfo::analyze(function, || DomTree::of(function));
    let one = info.top_level_loops().next().expect("a loop");
    f(&module.context, function, one)
}

const COUNTED: &str = "define i32 @f(i32 %n) {\nb0:\n  br label %h\nh:\n  %i = phi i32 [ 0, %b0 ], [ %j, %h ]\n  %j = add i32 %i, 1\n  %c = icmp ult i32 %j, %n\n  br i1 %c, label %h, label %x\nx:\n  ret i32 %j\n}\n";

/// `Loop::getCanonicalInductionVariable`: the phi from 0 by 1 is found, the
/// compare at the latch is its `icmp`, and an induction that steps by 2 is not
/// canonical.
#[test]
fn a_loop_from_zero_by_one_has_a_canonical_induction_variable() {
    only_loop(COUNTED, |context, function, one| {
        let phi = one.get_canonical_induction_variable(context, function).expect("%i");
        assert_eq!(function.instruction(phi).opcode, crate::opcode::Opcode::Phi);
        let cmp = one.get_latch_cmp_inst(function).expect("%c");
        assert!(matches!(function.instruction(cmp).opcode, crate::opcode::Opcode::ICmp(_)));
        assert!(one.get_incoming_and_back_edge(function).is_some());
    });
    let by_two = COUNTED.replace("%j = add i32 %i, 1", "%j = add i32 %i, 2");
    only_loop(&by_two, |context, function, one| {
        assert!(one.get_canonical_induction_variable(context, function).is_none());
    });
    let from_one = COUNTED.replace("[ 0, %b0 ]", "[ 1, %b0 ]");
    only_loop(&from_one, |context, function, one| {
        assert!(one.get_canonical_induction_variable(context, function).is_none());
    });
}

/// A latch that ends in a `ret`-less unconditional branch has no compare.
#[test]
fn a_latch_that_branches_unconditionally_has_no_latch_cmp() {
    let text = "define void @f(i1 %c) {\nb0:\n  br label %h\nh:\n  br i1 %c, label %l, label %x\nl:\n  br label %h\nx:\n  ret void\n}\n";
    only_loop(text, |_, function, one| assert!(one.get_latch_cmp_inst(function).is_none()));
}

/// `children` found the terminator again for every edge (`get_exiting_blocks`
/// cost 4.6x the pin's on d_faces): a switch of four edges took five lookups.
/// It takes one, and gives the edges of `child` with repeats in operand order.
#[test]
fn test_children_find_the_terminator_once_and_are_the_edges_of_child() {
    use crate::loopinfo::LoopGraph;
    use crate::module::TERMINATOR_LOOKUPS;

    let mut module = parse::module(
        "define i32 @f(i32 %x) {\nentry:\n  switch i32 %x, label %b [ i32 0, label %a i32 1, label %b i32 2, label %a ]\na:\n  br label %b\nb:\n  ret i32 0\n}\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let f = module.function_mut("f").expect("@f").1;
    for &block in f.layout() {
        let mut cursor = 0;
        let mut by_child = Vec::new();
        while let Some(next) = f.child(block, &mut cursor) {
            by_child.push(next);
        }
        assert_eq!(f.children(block).collect::<Vec<_>>(), by_child);
    }
    let entry = f.layout()[0];
    let before = TERMINATOR_LOOKUPS.with(std::cell::Cell::get);
    assert_eq!(f.children(entry).count(), 4, "default and three cases, repeats kept");
    assert_eq!(TERMINATOR_LOOKUPS.with(std::cell::Cell::get) - before, 1, "one lookup for four edges");
}

/// Swapping a branch's arms (what InstCombine does to invert a condition)
/// leaves the CFG as it was but walks it in another order, so the loop's blocks
/// are listed in another order. `LLRM_CHECK_PRESERVED` called the held
/// `LoopInfo` of 57 runs stale after InstCombine for it (x_collatz -O2); LLVM's
/// `verify` compares the sets.
#[test]
fn test_loopinfo_after_a_branch_swaps_its_arms_equals_the_fresh_one() {
    let mut module = parse::module(
        "define void @f(i16 %n, i1 %c) {
entry:
  br label %h

h:
  %i = phi i16 [ 0, %entry ], [ %j, %m ]
  br i1 %c, label %a, label %b

a:
  br label %m

b:
  br label %m

m:
  %j = add i16 %i, 1
  %t = icmp ult i16 %j, %n
  br i1 %t, label %h, label %exit

exit:
  ret void
}
",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let (_, function) = module.function_mut("f").expect("@f");
    let held = LoopInfo::analyze(&*function, || DomTree::of(function));
    let (a, b, h) = (block(function, "a"), block(function, "b"), block(function, "h"));
    let branch = function.terminator(h).expect("a branch");
    function.set_operand(branch, 1, crate::module::Operand::Block(b));
    function.set_operand(branch, 2, crate::module::Operand::Block(a));
    let fresh = LoopInfo::analyze(&*function, || DomTree::of(function));
    assert_eq!(held, fresh);
}

/// The same swap numbers two sibling loops in the other order: `shellsort` and
/// `fpbench` -O2 were reported stale after InstCombine for it.
#[test]
fn test_loopinfo_after_a_branch_swaps_two_loops_equals_the_fresh_one() {
    let mut module = parse::module(
        "define void @f(i1 %c, i1 %d) {
entry:
  br i1 %c, label %x, label %y

x:
  br i1 %d, label %x, label %end

y:
  br i1 %d, label %y, label %end

end:
  ret void
}
",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let (_, function) = module.function_mut("f").expect("@f");
    let held = LoopInfo::analyze(&*function, || DomTree::of(function));
    let (x, y, entry) = (block(function, "x"), block(function, "y"), block(function, "entry"));
    let branch = function.terminator(entry).expect("a branch");
    function.set_operand(branch, 1, crate::module::Operand::Block(y));
    function.set_operand(branch, 2, crate::module::Operand::Block(x));
    let fresh = LoopInfo::analyze(&*function, || DomTree::of(function));
    assert_eq!(held, fresh);
}
