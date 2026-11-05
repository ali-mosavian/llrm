use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitCondBrInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Br { c.visit_cond_br(inst) } else { Visit::None }
}

fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// SCEV and the loop passes read a guard as a plain compare: x_transpose's
/// `br (xor c, true)` guards hid `n > 0` from the trip count, which then came
/// out `smax(n, 1)` (the pin's IR has none).
#[test]
fn test_a_branch_on_a_not_swaps_its_successors() {
    let out = combined(
        "define void @f(i1 %c) {\nb0:\n  %n = xor i1 %c, true\n  br i1 %n, label %t, label %e\nt:\n  ret void\ne:\n  ret void\n}\n",
    );
    assert!(out.contains("br i1 %c, label %e, label %t"), "{out}");
}

/// Inverting `br (not c), ok, fail` left the `!prof` where it was, so the
/// check's failure edge kept the hot weight and the loop edge the 1: queens'
/// `place()` lost 87% in clocks once Nib checks carried weights.
#[test]
fn test_inverting_a_branch_swaps_its_weights_with_its_successors() {
    use crate::combine::combine_with_declared;
    let mut module = parsed(
        "define void @f(i1 %c) {\nb0:\n  %n = xor i1 %c, true\n  br i1 %n, label %t, label %e, !prof !0\nt:\n  ret void\ne:\n  ret void\n}\n!0 = !{!\"branch_weights\", i32 7, i32 1}\n",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    let line = out.lines().find(|l| l.contains("br i1 %c")).expect("the inverted branch");
    assert!(line.contains("label %e, label %t"), "{out}");
    let node = line.rsplit("!prof !").next().expect("a !prof").trim();
    assert!(out.contains(&format!("!{node} = !{{!\"branch_weights\", i32 1, i32 7}}")), "{out}");
}

/// `freelyInvertAllUsersOf`: the compare is inverted for a `not` of it, and a
/// branch on it swaps its arms and its weights.
#[test]
fn test_inverting_a_compare_for_a_not_swaps_the_weights_of_its_other_branch() {
    use crate::combine::combine_with_declared;
    fn visit_not(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        if c.function.instruction(inst).opcode == Opcode::Binary(llrm_mir::opcode::BinaryOp::Xor) {
            crate::combine_not::fold_not(c, inst).unwrap_or(Visit::None)
        } else {
            Visit::None
        }
    }
    let mut module = parsed(
        "define i1 @f(i16 %a, i16 %b) {\nb0:\n  %c = icmp slt i16 %a, %b\n  %n = xor i1 %c, true\n  br i1 %c, label %t, label %e, !prof !0\nt:\n  ret i1 %n\ne:\n  ret i1 %n\n}\n!0 = !{!\"branch_weights\", i32 7, i32 1}\n",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit_not, &mut declared);
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    let line = out.lines().find(|l| l.contains("br i1 %c")).expect("the branch");
    assert!(line.contains("label %e, label %t"), "{out}");
    let node = line.rsplit("!prof !").next().expect("a !prof").trim();
    assert!(out.contains(&format!("!{node} = !{{!\"branch_weights\", i32 1, i32 7}}")), "{out}");
}

#[test]
fn test_a_branch_to_one_block_on_both_edges_reads_no_condition() {
    let out = combined("define void @f(i1 %c) {\nb0:\n  br i1 %c, label %x, label %x\nx:\n  ret void\n}\n");
    assert!(!out.contains("br i1 %c"), "{out}");
}

#[test]
fn test_a_branch_on_a_plain_compare_is_left_alone() {
    let text = "define void @f(i32 %a) {\nb0:\n  %c = icmp sgt i32 %a, 0\n  br i1 %c, label %t, label %e\nt:\n  ret void\ne:\n  ret void\n}\n";
    assert!(combined(text).contains("br i1 %c, label %t, label %e"));
}

thread_local! {
    static SEEN: std::cell::RefCell<Vec<(String, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Records how many branches the cache holds for `%x` as each instruction is
/// visited.
fn record(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    use llrm_mir::module::Operand;
    let x = Operand::Value(c.function.parameters()[0]);
    let held = c.env.analyses.dom_conditions().map_or(0, |cache| cache.conditions_for(x).len());
    let kind = format!("{:?}", c.function.instruction(inst).opcode);
    SEEN.with(|seen| seen.borrow_mut().push((kind, held)));
    visit(c, inst)
}

/// InstCombine registers a branch with its `DomConditionCache` as it visits it
/// (`DC.registerBranch` ends `visitBranchInst`); ours registered every branch
/// before the first visit, so a compare visited ahead of the branch read a
/// condition the pin's had not yet seen.
#[test]
fn test_a_branch_is_known_to_the_cache_only_once_visited() {
    let mut module = parsed(
        "define i32 @f(i32 %x) {\nb0:\n  %d = icmp ult i32 %x, 20\n  %z = zext i1 %d to i32\n  %c = icmp ult i32 %x, 10\n  br i1 %c, label %t, label %e\nt:\n  %y = add i32 %x, %z\n  ret i32 %y\ne:\n  ret i32 0\n}\n",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    SEEN.with(|seen| seen.borrow_mut().clear());
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, record);
    let seen = SEEN.with(|seen| seen.borrow().clone());
    let held = |kind: &str| seen.iter().filter(|(k, _)| k.contains(kind)).map(|(_, held)| *held).collect::<Vec<_>>();
    assert_eq!(held("ICmp")[..2], [0, 0], "the compares ahead of the branch: {seen:?}");
    assert_eq!(held("Br")[0], 0, "the branch's own visit: {seen:?}");
    assert_eq!(held("Binary")[0], 1, "the add the branch dominates: {seen:?}");
}

/// x_insertion's second `br i1 %1` under the first one's true edge stayed a
/// branch the pin's InstCombine had made `br i1 true`: `decide@809` decided
/// it instead, 19 M Ir on d_faces -O2.
#[test]
fn test_a_second_branch_on_the_condition_below_an_edge_reads_a_number() {
    let out = combined(
        "define void @f(i1 %c) {\nb0:\n  br i1 %c, label %t, label %e\nt:\n  br i1 %c, label %x, label %y\nx:\n  ret void\ny:\n  ret void\ne:\n  ret void\n}\n",
    );
    assert!(out.contains("br i1 true, label %x, label %y"), "{out}");
    assert!(out.contains("br i1 %c, label %t, label %e"), "{out}");
}

/// A use the edge does not dominate keeps the condition: `m` is reached from
/// both arms.
#[test]
fn test_a_use_both_edges_reach_keeps_the_condition() {
    let text = "define void @f(i1 %c) {\nb0:\n  br i1 %c, label %t, label %e\nt:\n  br label %m\ne:\n  br label %m\nm:\n  br i1 %c, label %x, label %y\nx:\n  ret void\ny:\n  ret void\n}\n";
    assert!(combined(text).contains("br i1 %c, label %x, label %y"));
}

/// A phi reads the condition on the edge it comes in by.
#[test]
fn test_a_phi_input_on_an_edge_reads_a_number() {
    let out = combined(
        "define i1 @f(i1 %c) {\nb0:\n  br i1 %c, label %t, label %e\nt:\n  br label %m\ne:\n  br label %m\nm:\n  %p = phi i1 [ %c, %t ], [ %c, %e ]\n  ret i1 %p\n}\n",
    );
    assert!(out.contains("phi i1 [ true, %t ], [ false, %e ]"), "{out}");
}
