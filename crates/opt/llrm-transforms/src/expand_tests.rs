//! `Expander` against the interpreter, on the loops `llrm_analysis::generated` makes.

use llrm_analysis::generated::{Inputs, Rng, case, observed, seeds};
use llrm_analysis::induction::{self, Linear};
use llrm_analysis::memory::Unit;
use llrm_analysis::testing::layout;
use llrm_mir::edit::Position;
use llrm_mir::module::Operand;
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};

use super::{Expander, placed};

/// Every affine value of a generated loop with its form, by tracked index.
fn forms(text: &llrm_analysis::generated::Case) -> Vec<(usize, Linear, Linear)> {
    let module = text.module();
    let layout = layout(&module);
    let function = llrm_analysis::testing::function(&module, "f");
    let unit = Unit::of(&module, &layout, function);
    let loop_ = unit.shape().loops[0].clone();
    let counters = induction::basics(&unit, &loop_);
    let derived = induction::derived(&unit, &loop_, Some(&counters));
    let users = induction::users(&unit, &loop_, &counters, &derived);
    text.tracked
        .iter()
        .enumerate()
        .filter(|(_, one)| one.affine())
        .filter_map(|(at, one)| {
            let value = llrm_analysis::testing::value(function, &one.name);
            let of = users.values.get(&value).filter(|of| of.pointer.is_none())?;
            Some((at, of.start.clone(), of.step.clone()))
        })
        .collect()
}

/// Each recurrence's start and step, built before the loop, give at the
/// last trip what the loop computes there; and the loop gains no
/// instruction, so a product of invariants is a preheader multiply.
#[test]
fn test_generated_recurrences_expand_to_what_the_loop_computes() {
    let mut checked = 0;
    for seed in seeds() {
        let case = case(seed);
        let original = case.module();
        let mut rng = Rng::new(seed ^ 0x5eed);
        for (which, start, step) in forms(&case) {
            let mut module = case.module();
            let (context, function) = module.function_mut("f").expect("@f");
            let (b0, b1, b2) = (function.layout()[0], function.layout()[1], function.layout()[2]);
            let loop_size = function.block(b1).instructions().len();
            let mut expander = Expander::new(function.terminator(b0).expect("a branch"));
            let (first, by) = (expander.int(context, function, &start), expander.int(context, function, &step));
            assert_eq!(function.block(b1).instructions().len(), loop_size, "seed {seed}: expansion landed in the loop\n{}", case.text);
            let width = start.width;
            let ret = function.terminator(b2).expect("a return");
            let at = Position::Before(ret);
            let n = Operand::Value(function.parameters()[5]);
            let one = crate::counting::constant(context, &num_bigint::BigInt::from(1), 16);
            let wide = context.types.int(width);
            let narrow = context.types.int(16);
            let last = placed(context, function, Opcode::Binary(BinaryOp::Sub), narrow, vec![n, one], at);
            let last = if width == 32 { placed(context, function, Opcode::Cast(CastOp::ZExt), wide, vec![last], at) } else { last };
            let moved = placed(context, function, Opcode::Binary(BinaryOp::Mul), wide, vec![by, last], at);
            let sum = placed(context, function, Opcode::Binary(BinaryOp::Add), wide, vec![first, moved], at);
            let i32 = context.types.int(32);
            let result = if width == 32 { sum } else { placed(context, function, Opcode::Cast(CastOp::ZExt), i32, vec![sum], at) };
            function.set_operand(ret, 0, result);
            assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "seed {seed}\n{}", llrm_mir::print::module(&module));
            for _ in 0..3 {
                let (inputs, trip) = (Inputs::random(&mut rng), rng.below(40) as u16);
                let got = observed(&module, &inputs, which, trip).unwrap();
                let expected = observed(&original, &inputs, which, trip).unwrap();
                let mask = llrm_mir::context::mask(width);
                assert_eq!(got & mask, expected & mask, "seed {seed} %{} at trip {trip}, {inputs:?}\n{}", case.tracked[which].name, case.text);
                checked += 1;
            }
        }
    }
    eprintln!("expanded values checked {checked}");
    assert!(checked > 0);
}
