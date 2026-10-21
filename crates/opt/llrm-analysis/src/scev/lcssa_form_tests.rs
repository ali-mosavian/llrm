//! `formLCSSAForInstructions`.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{InstId, ValueDef};

use super::form_lcssa_for_instructions;

/// qcport host -Os: LSR's exit phi for a loop left by a switch with two cases
/// to one block had one input for its two edges, which the verifier refused.
#[test]
fn test_an_exit_entered_by_two_switch_cases_gets_a_phi_input_per_edge() {
    let mut module = llrm_mir::parse::module(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  switch i16 %next, label %b1 [
    i16 5, label %b2
    i16 9, label %b2
  ]

b2:
  %r = add i16 %next, %n
  ret i16 %r
}
",
    )
    .expect("a module");
    {
        let (context, function) = module.function_mut("f").expect("@f");
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let def: InstId = function
            .layout()
            .iter()
            .flat_map(|&block| function.iter_block(block))
            .find(|&inst| {
                function
                    .instruction(inst)
                    .result
                    .is_some_and(
                        |v| matches!(function.value(v).def, ValueDef::Instruction(_))
                            && function.value(v).name.as_deref() == Some("next"),
                    )
            })
            .expect("%next");
        form_lcssa_for_instructions(context, &DataLayout::default(), function, &loops, &tree, vec![def]);
    }
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
}
