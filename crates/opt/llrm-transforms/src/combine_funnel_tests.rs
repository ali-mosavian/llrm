use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::Declared;

use crate::combine::{Combiner, Visit, combine_with_declared};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitOr`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Or) {
        crate::combine_or::visit_or(c, inst)
    } else {
        Visit::None
    }
}

/// `@f(params)` returning i32 after `visitOr` alone, its declarations placed.
fn combined(
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define i32 @f({params}) {{\nb0:\n{body}  ret i32 %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).expect("placed");
    printed(&module)
}

/// Shapes pinned `opt -passes=instcombine` gives for these inputs.
#[test]
fn shifts_whose_constant_amounts_sum_to_the_width_are_a_funnel_shift() {
    for body in [
        "  %a = shl i32 %x, 5\n  %b = lshr i32 %y, 27\n  %r = or i32 %a, %b\n",
        "  %a = lshr i32 %y, 27\n  %b = shl i32 %x, 5\n  %r = or i32 %a, %b\n",
    ] {
        let out = combined("i32 %x, i32 %y", body);
        assert!(out.contains("call i32 @llvm.fshl.i32(i32 %x, i32 %y, i32 5)"), "{out}");
        assert!(!out.contains("shl i32") && !out.contains("lshr i32"), "{out}");
    }
}

#[test]
fn shifts_that_sum_to_less_than_the_width_are_left() {
    let out = combined("i32 %x, i32 %y", "  %a = shl i32 %x, 5\n  %b = lshr i32 %y, 20\n  %r = or i32 %a, %b\n");
    assert!(out.contains("shl i32 %x, 5") && !out.contains("llvm.fsh"), "{out}");
}

/// A rotate by a masked variable amount and by its masked negation.
#[test]
fn a_rotate_by_a_masked_amount_and_its_negation_is_a_funnel_shift() {
    let out = combined(
        "i32 %x, i32 %z",
        "  %k = and i32 %z, 31\n  %n = sub i32 0, %z\n  %m = and i32 %n, 31\n  %a = shl i32 %x, %k\n  %b = lshr i32 %x, %m\n  %r = or i32 %a, %b\n",
    );
    assert!(out.contains("call i32 @llvm.fshl.i32(i32 %x, i32 %x, i32 %z)"), "{out}");
}

/// `Width - k` with `k` known below the width: `fshl` when the left shift
/// takes `k`, `fshr` when the right one does.
#[test]
fn a_shift_by_k_and_by_width_minus_k_is_a_funnel_shift_either_way() {
    let out = combined(
        "i32 %x, i32 %y, i32 %z",
        "  %k = and i32 %z, 15\n  %n = sub i32 32, %k\n  %a = shl i32 %x, %k\n  %b = lshr i32 %y, %n\n  %r = or i32 %a, %b\n",
    );
    assert!(out.contains("call i32 @llvm.fshl.i32(i32 %x, i32 %y, i32 %k)"), "{out}");
    let out = combined(
        "i32 %x, i32 %y, i32 %z",
        "  %k = and i32 %z, 15\n  %n = sub i32 32, %k\n  %a = lshr i32 %y, %k\n  %b = shl i32 %x, %n\n  %r = or i32 %a, %b\n",
    );
    assert!(out.contains("call i32 @llvm.fshr.i32(i32 %x, i32 %y, i32 %k)"), "{out}");
}

/// Without a bound on `k` the amount can be the width and `Width - k` is not a
/// funnel shift's amount: the shifts stay.
#[test]
fn a_shift_by_an_unbounded_amount_and_its_complement_is_left() {
    let out = combined(
        "i32 %x, i32 %y, i32 %z",
        "  %n = sub i32 32, %z\n  %a = shl i32 %x, %z\n  %b = lshr i32 %y, %n\n  %r = or i32 %a, %b\n",
    );
    assert!(!out.contains("llvm.fsh"), "{out}");
}
