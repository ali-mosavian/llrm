use std::cell::Cell;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visitSub`'s call: `0 - x` is `Negate(true, nsw, x)`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    let instruction = c.function.instruction(inst);
    if instruction.opcode != Opcode::Binary(BinaryOp::Sub) {
        return Visit::None;
    }
    let (zero, x, nsw) = (instruction.operands[0], instruction.operands[1], instruction.flags.contains(Flags::NSW));
    if !matches!(
        zero,
        Operand::Constant(k) if c.context.get(k).kind == llrm_mir::context::ConstantKind::Int(0)
    ) {
        return Visit::None;
    }
    match c.negate(true, nsw, x) {
        Some(negated) => Visit::Replace(negated),
        None => Visit::None,
    }
}

fn run(
    visit: fn(&mut Combiner, InstId) -> Visit,
    text: &str,
) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `define RET @f(PARAMS)` with `body` before and `after` after `0 - x`.
fn check(
    ret: &str,
    params: &str,
    body: &str,
    after: &str,
) {
    let text = format!("define {ret} @f({params}) {{\nb0:\n{body}}}\n");
    let want = format!("define {ret} @f({params}) {{\nb0:\n{after}}}\n");
    assert_eq!(run(visit, &text), want);
}

/// `-(x + 1)` is `~x` (`opt` agrees).
#[test]
fn neg_of_inc() {
    check(
        "i32",
        "i32 %x",
        "  %a = add i32 %x, 1\n  %r = sub i32 0, %a\n  ret i32 %r\n",
        "  %0 = xor i32 %x, -1\n  ret i32 %0\n",
    );
}

/// `-(~x)` is `x + 1`.
#[test]
fn neg_of_not() {
    check(
        "i32",
        "i32 %x",
        "  %a = xor i32 %x, -1\n  %r = sub i32 0, %a\n  ret i32 %r\n",
        "  %0 = add i32 %x, 1\n  ret i32 %0\n",
    );
}

/// `-(a - b)` is `b - a`.
#[test]
fn neg_of_sub() {
    check(
        "i32",
        "i32 %a, i32 %b",
        "  %s = sub i32 %a, %b\n  %r = sub i32 0, %s\n  ret i32 %r\n",
        "  %0 = sub i32 %b, %a\n  ret i32 %0\n",
    );
}

/// `-(x >>s 31)` is `x >>u 31`.
#[test]
fn neg_of_ashr_sign_smear() {
    check(
        "i32",
        "i32 %x",
        "  %s = ashr i32 %x, 31\n  %r = sub i32 0, %s\n  ret i32 %r\n",
        "  %0 = lshr i32 %x, 31\n  ret i32 %0\n",
    );
}

/// `-(zext i1 x)` is `sext i1 x`.
#[test]
fn neg_of_zext_bool() {
    check(
        "i32",
        "i1 %x",
        "  %s = zext i1 %x to i32\n  %r = sub i32 0, %s\n  ret i32 %r\n",
        "  %0 = sext i1 %x to i32\n  ret i32 %0\n",
    );
}

/// `-(select c, 3, 5)` negates both arms.
#[test]
fn neg_of_select_of_constants() {
    check(
        "i32",
        "i1 %c",
        "  %s = select i1 %c, i32 3, i32 5\n  %r = sub i32 0, %s\n  ret i32 %r\n",
        "  %0 = select i1 %c, i32 -3, i32 -5\n  ret i32 %0\n",
    );
}

/// `-(x * 7)` is `-7 * x` (the negated operand first, as `Negator` builds it;
/// `opt` commutes it later).
#[test]
fn neg_of_mul_by_constant() {
    check(
        "i32",
        "i32 %x",
        "  %m = mul i32 %x, 7\n  %r = sub i32 0, %m\n  ret i32 %r\n",
        "  %0 = mul i32 -7, %x\n  ret i32 %0\n",
    );
}

/// `-(x << 3)` is `x * -8` for a true negation.
#[test]
fn neg_of_shl_by_constant_is_a_mul() {
    check(
        "i32",
        "i32 %x",
        "  %m = shl i32 %x, 3\n  %r = sub i32 0, %m\n  ret i32 %r\n",
        "  %0 = mul i32 %x, -8\n  ret i32 %0\n",
    );
}

/// `-(zext (x >>u 7))` is `sext (x >>s 7)`.
#[test]
fn neg_of_zext_of_the_sign_bit() {
    check(
        "i32",
        "i8 %x",
        "  %l = lshr i8 %x, 7\n  %z = zext i8 %l to i32\n  %r = sub i32 0, %z\n  ret i32 %r\n",
        "  %0 = ashr i8 %x, 7\n  %1 = sext i8 %0 to i32\n  ret i32 %1\n",
    );
}

/// `-((x >>u 5) & 1)` is `(x << 26) >>s 31`.
#[test]
fn neg_of_the_low_bit_is_a_shift_pair() {
    check(
        "i32",
        "i32 %x",
        "  %l = lshr i32 %x, 5\n  %a = and i32 %l, 1\n  %r = sub i32 0, %a\n  ret i32 %r\n",
        "  %0 = shl i32 %x, 26\n  %1 = ashr i32 %0, 31\n  ret i32 %1\n",
    );
}

/// `-(x /s 3)` is `x /s -3`.
#[test]
fn neg_of_sdiv_by_constant() {
    check(
        "i32",
        "i32 %x",
        "  %d = sdiv i32 %x, 3\n  %r = sub i32 0, %d\n  ret i32 %r\n",
        "  %0 = sdiv i32 %x, -3\n  ret i32 %0\n",
    );
}

/// `-(x + -y)` is `y - x`.
#[test]
fn neg_of_add_with_a_negation() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %n = sub i32 0, %y\n  %a = add i32 %x, %n\n  %r = sub i32 0, %a\n  ret i32 %r\n",
        "  %0 = sub i32 %y, %x\n  ret i32 %0\n",
    );
}

/// `-((x*3) + y)` is `(-3 * x) - y` for a true negation.
#[test]
fn neg_of_add_with_one_negatible_operand() {
    check(
        "i32",
        "i32 %x, i32 %y",
        "  %m = mul i32 %x, 3\n  %a = add i32 %m, %y\n  %r = sub i32 0, %a\n  ret i32 %r\n",
        "  %0 = mul i32 -3, %x\n  %1 = sub i32 %0, %y\n  ret i32 %1\n",
    );
}

/// `-(x ^ 12)` is `(x ^ -13) + 1`.
#[test]
fn neg_of_xor_with_a_constant() {
    check(
        "i32",
        "i32 %x",
        "  %a = xor i32 %x, 12\n  %r = sub i32 0, %a\n  ret i32 %r\n",
        "  %0 = xor i32 %x, -13\n  %1 = add i32 %0, 1\n  ret i32 %1\n",
    );
}

/// `-(trunc x + 1)` is `~trunc x`.
#[test]
fn neg_of_trunc() {
    check(
        "i16",
        "i32 %x",
        "  %t = trunc i32 %x to i16\n  %a = add i16 %t, 1\n  %r = sub i16 0, %a\n  ret i16 %r\n",
        "  %t = trunc i32 %x to i16\n  %0 = xor i16 %t, -1\n  ret i16 %0\n",
    );
}

/// `Negate`'s `IsNSW`: `-(a -nsw b)` is `b -nsw a` only for an `nsw` negation.
#[test]
fn neg_of_an_nsw_sub_keeps_nsw_only_for_an_nsw_negation() {
    check(
        "i32",
        "i32 %a, i32 %b",
        "  %s = sub nsw i32 %a, %b\n  %r = sub nsw i32 0, %s\n  ret i32 %r\n",
        "  %0 = sub nsw i32 %b, %a\n  ret i32 %0\n",
    );
    check(
        "i32",
        "i32 %a, i32 %b",
        "  %s = sub nsw i32 %a, %b\n  %r = sub i32 0, %s\n  ret i32 %r\n",
        "  %0 = sub i32 %b, %a\n  ret i32 %0\n",
    );
}

/// A value with other uses is negated only for a true negation, and only
/// where it needs no recursion: `-(x*3)` with `x*3` read again is left alone
/// when the negation is not `0 - ...`.
#[test]
fn a_multi_use_product_is_not_negated_by_a_plain_negate() {
    thread_local!(static RESULT: Cell<Option<bool>> = const { Cell::new(None) });
    fn probe(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        let instruction = c.function.instruction(inst);
        if instruction.opcode == Opcode::Binary(BinaryOp::Sub) {
            let x = instruction.operands[1];
            RESULT.with(|r| r.set(Some(c.negate(false, false, x).is_some())));
        }
        Visit::None
    }
    let text = "define i32 @f(i32 %x, i32 %y) {\nb0:\n  %m = mul i32 %x, 3\n  %s = sub i32 %y, %m\n  %u = add i32 %m, 1\n  %r = add i32 %s, %u\n  ret i32 %r\n}\n";
    run(probe, text);
    assert_eq!(RESULT.with(Cell::get), Some(false));
}

/// A failed negation erases what it made (`run`'s cleanup): the function
/// after a refused `Negate` has the instructions it started with.
#[test]
fn a_refused_negation_leaves_no_instruction_behind() {
    thread_local!(static COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) });
    fn probe(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        let instruction = c.function.instruction(inst);
        if instruction.opcode == Opcode::Binary(BinaryOp::Sub) {
            let x = instruction.operands[1];
            let before = c.function.iter_block(c.function.parent(inst).expect("placed")).count();
            let negated = c.negate(false, false, x);
            let after = c.function.iter_block(c.function.parent(inst).expect("placed")).count();
            assert!(negated.is_none());
            COUNTS.with(|r| r.set((before, after)));
        }
        Visit::None
    }
    // x*3 is negatible, y is not: the add is refused after `x * -3` is made.
    let text = "define i32 @f(i32 %x, i32 %y, i32 %z) {\nb0:\n  %m = mul i32 %x, 3\n  %a = add i32 %m, %y\n  %s = sub i32 %z, %a\n  ret i32 %s\n}\n";
    run(probe, text);
    let (before, after) = COUNTS.with(Cell::get);
    assert!(before > 0);
    assert_eq!(before, after);
}
