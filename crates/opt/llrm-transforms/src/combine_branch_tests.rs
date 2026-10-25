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
