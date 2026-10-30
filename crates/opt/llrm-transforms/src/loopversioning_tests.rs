//! `LoopVersioning` against the pinned LLVM's
//! `opt -passes='loop-simplify,lcssa,loop-versioning'` on the same loops.
//! The pin's blocks are compared as `shape`s: each block's instruction kinds
//! and successors, without the names of values and layout order (the pin
//! numbers its second `.loopexit` `2`, where MIR numbers it `1`).

use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{InstId, MetadataId, MetadataOperand, Module, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

use super::{LoopVersioning, find_defs_used_outside_of_loop, form_dedicated_exit_blocks, is_recursively_lcssa_form};
use crate::runtimechecks::tests::{Analysed, COPY as COPY_WITH_PH, analysed};

/// `a[i] = b[i]` in simplify form: a preheader and a dedicated exit.
const COPY: &str = "define void @f(ptr %a, ptr %b, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %ph, label %exit
ph:
  br label %loop
loop:
  %i = phi i64 [ 0, %ph ], [ %i.next, %loop ]
  %pb = getelementptr inbounds i32, ptr %b, i64 %i
  %v = load i32, ptr %pb, align 4
  %pa = getelementptr inbounds i32, ptr %a, i64 %i
  store i32 %v, ptr %pa, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit.loopexit, label %loop
exit.loopexit:
  br label %exit
exit:
  ret void
}
";

/// A running sum stored on each trip, read after the loop through an exit phi.
const SUM: &str = "define i32 @f(ptr %a, ptr %b, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %ph, label %exit
ph:
  br label %loop
loop:
  %i = phi i64 [ 0, %ph ], [ %i.next, %loop ]
  %s = phi i32 [ 0, %ph ], [ %s.next, %loop ]
  %pa = getelementptr inbounds i32, ptr %a, i64 %i
  %v = load i32, ptr %pa, align 4
  %s.next = add i32 %s, %v
  %pb = getelementptr inbounds i32, ptr %b, i64 %i
  store i32 %s.next, ptr %pb, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit.loopexit, label %loop
exit.loopexit:
  %s.lcssa = phi i32 [ %s.next, %loop ]
  br label %exit
exit:
  %r = phi i32 [ 0, %entry ], [ %s.lcssa, %exit.loopexit ]
  ret i32 %r
}
";

/// `text`'s loop `loop` versioned and annotated.
fn versioned(text: &str) -> (Analysed, Module) {
    let mut a = analysed(text);
    let (header, checks, memory) = (a.header, a.rtc.checks.clone(), a.memory.clone());
    a.rewrite(|rw, rtc| {
        let mut versioning = LoopVersioning::new(rtc, &checks, &memory, header);
        versioning.version_loop(rw);
        versioning.annotate_loop_with_no_alias(rw);
    });
    let module = a.finished();
    (a, module)
}

/// The loops of the versioned function, all in simplify and LCSSA form: how
/// many.
fn simplified_loops(a: &Analysed) -> usize {
    let tree = DomTree::of(&a.function);
    let loops = LoopInfo::analyze(&a.function, || &tree);
    let all = loops.get_loops_in_preorder();
    for lp in &all {
        let text = llrm_mir::print::body(&a.context, &a.function);
        assert!(lp.is_loop_simplify_form(&a.function), "{text}");
        assert!(is_recursively_lcssa_form(&a.function, &loops, &tree, *lp), "{text}");
    }
    all.len()
}

/// One line per block, by name less trailing digits: its instruction kinds,
/// then `->` and the names of its successors.
fn shape(a: &Analysed) -> Vec<String> {
    let name = |block| a.function.block(block).name.clone().expect("a named block");
    let trim = |name: String| name.trim_end_matches(|c: char| c.is_ascii_digit()).to_owned();
    let mut lines: Vec<String> = a
        .function
        .layout()
        .iter()
        .map(|&block| {
            let kinds: Vec<String> = a
                .lines(block)
                .iter()
                .map(|line| {
                    line.split(" = ").last().expect("a line").split_whitespace().next().expect("a word").to_owned()
                })
                .collect();
            let successors: Vec<String> = a.function.successors(block).into_iter().map(|s| trim(name(s))).collect();
            format!("{}: {} -> {}", trim(name(block)), kinds.join(" "), successors.join(" "))
        })
        .collect();
    lines.sort();
    lines
}

/// The node `id` of `module`: distinct or not, and its operands.
fn node(
    module: &Module,
    id: MetadataId,
) -> (bool, Vec<MetadataOperand>) {
    let node = &module.metadata[id.0 as usize];
    (node.distinct, node.operands.clone())
}

/// The instruction whose result is named `name`.
fn named(
    a: &Analysed,
    name: &str,
) -> InstId {
    a.function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| {
            a.function.instruction(inst).result.is_some_and(|v| a.function.value(v).name.as_deref() == Some(name))
        })
        .expect(name)
}

/// The store of the block named `block`.
fn store_in(
    a: &Analysed,
    block: &str,
) -> InstId {
    a.function
        .iter_block(a.block(block))
        .find(|&inst| matches!(a.function.instruction(inst).opcode, Opcode::Store { .. }))
        .expect("a store")
}

/// The metadata `kind` of `inst`.
fn metadata(
    a: &Analysed,
    inst: InstId,
    kind: &str,
) -> Option<MetadataId> {
    a.function.instruction(inst).metadata.iter().find(|(one, _)| one == kind).map(|&(_, id)| id)
}

/// The pin, for `a[i] = b[i]`: a check block, the fall-back loop and the
/// original, two exits, one for each loop, joined at the old exit.
#[test]
fn a_copy_loop_is_versioned_into_two_simplified_loops() {
    let (a, module) = versioned(COPY);
    crate::testing::printed(&module);
    assert_eq!(simplified_loops(&a), 2);
    let mut pin: Vec<String> = [
            "entry: icmp br -> loop.lver.check exit",
            "exit: ret -> ",
            "exit.loopexit: br -> exit",
            "exit.loopexit.loopexit: br -> exit.loopexit",
            "exit.loopexit.loopexit: br -> exit.loopexit",
            "loop: phi getelementptr load getelementptr store add icmp br -> exit.loopexit.loopexit loop",
            "loop.lver.check: shl getelementptr getelementptr icmp icmp and br -> loop.ph.lver.orig loop.ph",
            "loop.lver.orig: phi getelementptr load getelementptr store add icmp br -> exit.loopexit.loopexit loop.lver.orig",
            "loop.ph: br -> loop",
            "loop.ph.lver.orig: br -> loop.lver.orig",
        ]
    .iter()
    .map(ToString::to_string)
    .collect();
    pin.sort();
    assert_eq!(shape(&a), pin);
}

/// The pin, for a sum read after the loop: the exit phi gets an arm from each
/// loop, through the phis of the two new exits.
#[test]
fn a_value_read_after_the_loop_comes_from_either_copy() {
    let (a, module) = versioned(SUM);
    crate::testing::printed(&module);
    assert_eq!(simplified_loops(&a), 2);
    let shape = shape(&a);
    assert!(shape.contains(&"exit.loopexit.loopexit: phi br -> exit.loopexit".to_owned()), "{shape:#?}");
    let exit = a.block("exit.loopexit");
    let lines = a.lines(exit);
    assert!(lines[0].starts_with("%s.lcssa = phi i32 [ %s.lcssa.ph, %exit.loopexit.loopexit"), "{lines:#?}");
    assert!(lines[0].contains("%exit.loopexit.loopexit1 ]"), "{lines:#?}");
    // The pin's second arm reads the copy's value.
    let copy = a.lines(a.block("exit.loopexit.loopexit"));
    assert_eq!(copy[0], "%s.lcssa.ph = phi i32 [ %s.next.lver.orig, %loop.lver.orig ]");
}

/// The pin: the load's group scope; the store's group scope and, as
/// `noalias`, the load's: the groups were checked against each other. Each
/// scope is a distinct node of the `LVerDomain` domain that names itself.
#[test]
fn the_memory_instructions_get_the_scopes_of_their_checked_groups() {
    let (a, module) = versioned(COPY);
    let (v, store) = (named(&a, "v"), store_in(&a, "loop"));
    let load = metadata(&a, v, "alias.scope").expect("the load's scope list");
    let store_noalias = metadata(&a, store, "noalias").expect("the store's noalias list");
    let store = metadata(&a, store, "alias.scope").expect("the store's scope list");
    assert_eq!(metadata(&a, v, "noalias"), None);
    assert_eq!(store_noalias, load, "the store does not alias what the load's group is");
    assert_ne!(load, store);
    let (distinct, scopes) = node(&module, load);
    assert!(!distinct && scopes.len() == 1);
    let MetadataOperand::Node(scope) = scopes[0] else { panic!("a scope") };
    let (distinct, operands) = node(&module, scope);
    assert!(distinct);
    assert_eq!(operands[0], MetadataOperand::Node(scope), "a scope names itself first");
    let MetadataOperand::Node(domain) = operands[1] else { panic!("a domain") };
    assert_eq!(
        node(&module, domain),
        (true, vec![MetadataOperand::Node(domain), MetadataOperand::String("LVerDomain".to_owned())])
    );
    // The fall-back loop is not annotated.
    assert_eq!(metadata(&a, named(&a, "v.lver.orig"), "alias.scope"), None);
}

/// A scope list an instruction already has is extended with its group's, not
/// replaced (`MDNode::concatenate`).
#[test]
fn a_scope_list_the_instruction_has_is_extended() {
    let text = COPY.replace("align 4\n  %pa", "align 4, !alias.scope !10\n  %pa")
        + "!10 = !{!11}\n!11 = distinct !{!11, !12}\n!12 = distinct !{!12, !\"Other\"}\n";
    let (a, module) = versioned(&text);
    let list = metadata(&a, named(&a, "v"), "alias.scope").expect("a scope list");
    let (_, operands) = node(&module, list);
    assert_eq!(operands.len(), 2, "{operands:?}");
    // The module's nodes are numbered from 0 as they are read.
    assert_eq!(operands[0], MetadataOperand::Node(MetadataId(1)));
    assert_ne!(operands[1], operands[0]);
}

/// The pin does not make a phi for a value the exit already takes from the
/// loop; a value it does not take gets `<name>.lver` with an arm from each
/// loop.
#[test]
fn a_value_without_an_exit_phi_gets_one_with_an_arm_from_each_loop() {
    let mut a = analysed(COPY);
    let (header, checks, memory) = (a.header, a.rtc.checks.clone(), a.memory.clone());
    let v = a
        .function
        .walk()
        .find(|&(_, inst)| matches!(a.function.instruction(inst).opcode, Opcode::Load { .. }))
        .expect("the load")
        .1;
    a.rewrite(|rw, rtc| {
        let mut versioning = LoopVersioning::new(rtc, &checks, &memory, header);
        versioning.version_loop_with(rw, &[v]);
    });
    let lines = a.lines(a.block("exit.loopexit"));
    assert_eq!(
        lines[0],
        "%v.lver = phi i32 [ %v.lver.ph, %exit.loopexit.loopexit ], [ %v.lver.ph1, %exit.loopexit.loopexit1 ]",
        "{lines:#?}"
    );
}

/// The values a loop defines and reads outside it: the sum's.
#[test]
fn the_values_read_outside_the_loop_are_found() {
    let a = analysed(&SUM.replace("%s.lcssa = phi i32 [ %s.next, %loop ]", "%s.lcssa = phi i32 [ %s.next, %loop ]"));
    let lp = a.loops.get_loop_for(a.header).expect("the loop");
    let defs = find_defs_used_outside_of_loop(&a.function, lp);
    let names: Vec<_> = defs
        .iter()
        .map(|&d| a.function.value(a.function.instruction(d).result.expect("a value")).name.clone())
        .collect();
    assert_eq!(names, [Some("s.next".to_owned())]);
}

/// A loop value read outside the loop but through no exit phi is not in
/// LCSSA form; with the phi it is.
#[test]
fn a_use_outside_the_loop_without_a_phi_is_not_lcssa() {
    let open = SUM.replace("%r = phi i32 [ 0, %entry ], [ %s.lcssa, %exit.loopexit ]", "%r = add i32 %s.next, 1");
    let a = analysed(&open.replace("  %s.lcssa = phi i32 [ %s.next, %loop ]\n", ""));
    let lp = a.loops.get_loop_for(a.header).expect("the loop");
    assert!(!is_recursively_lcssa_form(&a.function, &a.loops, &a.tree, lp));
    let a = analysed(SUM);
    let lp = a.loops.get_loop_for(a.header).expect("the loop");
    assert!(is_recursively_lcssa_form(&a.function, &a.loops, &a.tree, lp));
}

/// An exit that other blocks also reach gets a block of its own; one that
/// only the loop reaches is left, and nothing changes the second time.
#[test]
fn only_a_shared_exit_is_split() {
    let mut a = analysed(COPY_WITH_PH);
    let blocks: Vec<_> = a.loops.get_loop_for(a.header).expect("the loop").get_blocks().to_vec();
    assert!(form_dedicated_exit_blocks(&mut a.function, &blocks), "`exit` is also reached from `entry`");
    assert!(!form_dedicated_exit_blocks(&mut a.function, &blocks));
    let exit = a.block("exit.loopexit");
    assert_eq!(a.function.predecessors(a.block("exit")).len(), 2);
    assert_eq!(a.function.predecessors(exit).len(), 1);
}

/// Equal scope lists are one node, found among the module's too.
#[test]
fn a_tuple_is_made_once() {
    let mut a = analysed(COPY);
    let (header, checks, memory) = (a.header, a.rtc.checks.clone(), a.memory.clone());
    let (first, second) = a.rewrite(|rw, rtc| {
        let mut versioning = LoopVersioning::new(rtc, &checks, &memory, header);
        let ops = vec![MetadataOperand::String("x".to_owned())];
        (versioning.tuple(rw, ops.clone()), LoopVersioning::new(rtc, &checks, &memory, header).tuple(rw, ops))
    });
    assert_eq!(first, second);
}

/// The fall-back loop has no annotation until one is asked for with the
/// original instruction: `annotateInstWithNoAlias(Versioned, Orig)`.
#[test]
fn an_instruction_is_annotated_from_the_one_it_was_cloned_from() {
    let mut a = analysed(COPY);
    let (header, checks, memory) = (a.header, a.rtc.checks.clone(), a.memory.clone());
    let load = memory[0];
    a.rewrite(|rw, rtc| {
        let mut versioning = LoopVersioning::new(rtc, &checks, &memory, header);
        versioning.version_loop(rw);
        versioning.prepare_no_alias_metadata(rw);
        let copy = match versioning.vmap.values[&rw.function.instruction(load).result.expect("a value")] {
            Operand::Value(v) => match rw.function.value(v).def {
                ValueDef::Instruction(i) => i,
                _ => unreachable!("a clone"),
            },
            _ => unreachable!("a clone"),
        };
        versioning.annotate_inst_with_no_alias(rw, copy, load);
    });
    let copy = named(&a, "v.lver.orig");
    assert!(metadata(&a, copy, "alias.scope").is_some());
    assert_eq!(metadata(&a, named(&a, "v"), "alias.scope"), None);
}

/// Without a memory check (and no SCEV predicate, DEVIATE 29) there is
/// nothing to branch on, and versioning refuses.
#[test]
#[should_panic(expected = "called even though we don't need any runtime checks")]
fn a_loop_with_no_check_is_not_versioned() {
    let mut a = analysed(COPY);
    let (header, memory) = (a.header, a.memory.clone());
    a.rewrite(|rw, rtc| LoopVersioning::new(rtc, &[], &memory, header).version_loop(rw));
}
