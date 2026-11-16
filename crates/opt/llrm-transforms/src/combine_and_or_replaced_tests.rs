use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_and::visit_and;
use crate::combine_or::visit_or;
use crate::testing::{parsed, printed};

/// `visit` that does only `visitAnd` and `visitOr`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::Or) => visit_or(c, inst),
        Opcode::Binary(BinaryOp::And) => visit_and(c, inst),
        _ => Visit::None,
    }
}

/// `@f` after `visitAnd` and `visitOr` alone.
fn run(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// known_loads_walks -O2 kept `(x ^ 7) | x` (the pin: `x | 7`): one `xor` more
/// per trip of a hot select, 164 instructions over the pin.
#[test]
fn an_or_reads_its_other_operand_as_zero_inside_the_xor() {
    let out = run("define i8 @f(i8 %x) {\nb0:\n  %a = xor i8 %x, 7\n  %r = or i8 %a, %x\n  ret i8 %r\n}\n");
    assert!(out.contains("or i8 %x, 7") && !out.contains("xor"), "{out}");
}

/// The `and` side reads it as all ones: `(x ^ y) & x` is `x & ~y`.
#[test]
fn an_and_reads_its_other_operand_as_all_ones_inside_the_xor() {
    let out = run("define i8 @f(i8 %x, i8 %y) {\nb0:\n  %a = xor i8 %x, %y\n  %r = and i8 %a, %x\n  ret i8 %r\n}\n");
    assert!(out.contains("xor i8 -1, %y") && out.contains("and i8 %x, %0"), "{out}");
}

/// A xor with another user is not rebuilt (`SimplifyOnly`), only replaced when
/// it simplifies: `(x ^ y) | x` has nothing to simplify to, so both stay.
#[test]
fn a_shared_operation_that_does_not_simplify_is_left() {
    let text = "define i8 @f(i8 %x, i8 %y) {\nb0:\n  %a = xor i8 %x, %y\n  %r = or i8 %a, %x\n  %s = add i8 %r, %a\n  ret i8 %s\n}\n";
    assert!(run(text).contains("xor i8 %x, %y"), "{}", run(text));
}
