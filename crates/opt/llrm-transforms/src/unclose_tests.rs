//! The loop-closing phis the pipeline's last step opens again.

use llrm_mir::opcode::Opcode;
use llrm_mir::program::Program;

use crate::pipeline::{self, Applied};
use crate::testing::parsed;

/// A loop with a second way out, its counter read after both.
const TWO_EXITS: &str = "define i16 @f(ptr %a, i16 %n, i16 %k) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %b2, label %b4

b2:
  %at = mul i16 %i, 2
  %cell = getelementptr inbounds i8, ptr %a, i16 %at
  %v = load i16, ptr %cell
  %big = icmp sgt i16 %v, %k
  br i1 %big, label %b5, label %b3

b3:
  %next = add nsw i16 %i, 1
  br label %b1

b4:
  br label %b6

b5:
  br label %b6

b6:
  %r = add nsw i16 %i, 7
  ret i16 %r
}
";

/// Every phi of @f in `module`, with the distinct values its arms name.
fn phis(module: &llrm_mir::module::Module) -> Vec<usize> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    function
        .walk()
        .filter(|&(_, one)| function.instruction(one).opcode == Opcode::Phi)
        .map(|(_, one)| {
            let mut values: Vec<_> = function.instruction(one).operands.chunks(2).map(|pair| pair[0]).collect();
            values.sort_by_key(|value| format!("{value:?}"));
            values.dedup();
            values.len()
        })
        .collect()
}

/// Loop closing left `phi [%i, %b4], [%i, %b5]` and one-armed `phi [%i, %b4]`
/// in the optimized body, and isel turned each into a LIR phi of one value:
/// SsaSpill spilled it as a value of its own (deedlines PLASMABLOBS, +7%).
#[test]
fn test_the_pipeline_leaves_no_phi_of_one_value() {
    let mut module = parsed(TWO_EXITS);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_code16::Dos::default()), |program| pipeline::applied(program, &Applied::default()))
        .and_then(|done| done)
        .expect("optimizes");
    let kinds = phis(&module);
    assert!(!kinds.is_empty(), "premise: the loop's counter is a phi");
    assert!(kinds.iter().all(|distinct| *distinct > 1), "a phi of one value remains: {kinds:?}\n{}", llrm_mir::print::module(&module));
}
