//! Each case is a function pinned `opt -passes=instcombine` (i486, `-sse`)
//! rewrites with `foldBinOpShiftWithShift`; `visit` runs the four `visit*`
//! that call it and nothing else. Before it was wired in, each kept both
//! shifts.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::Binary(BinaryOp::Add) => crate::combine_addsub::visit_add(c, inst),
        Opcode::Binary(BinaryOp::And) => crate::combine_and::visit_and(c, inst),
        Opcode::Binary(BinaryOp::Or) => crate::combine_or::visit_or(c, inst),
        Opcode::Binary(BinaryOp::Xor) => crate::combine_xor::visit_xor(c, inst),
        _ => Visit::None,
    }
}

/// `@f(params)` returning i32 after the four `visit*`.
fn combined(
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("define i32 @f({params}) {{\nb0:\n{body}  ret i32 %r\n}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `body` comes out with each of `wanted` and none of `unwanted`.
fn check(
    params: &str,
    body: &str,
    wanted: &[&str],
    unwanted: &[&str],
) {
    let out = combined(params, body);
    for one in wanted {
        assert!(out.contains(one), "`{one}` missing\n{out}");
    }
    for one in unwanted {
        assert!(!out.contains(one), "`{one}` present\n{out}");
    }
}

/// `((x << 4) | m) | (y << 4)` kept two shifts; it is `((x | y) << 4) | m`.
#[test]
fn an_or_of_two_shifts_by_one_amount_shifts_the_or() {
    check(
        "i32 %x, i32 %y, i32 %m",
        "  %a = shl i32 %x, 4\n  %b = or i32 %a, %m\n  %c = shl i32 %y, 4\n  %r = or i32 %b, %c\n",
        &["or i32 %x, %y", "shl i32 %", ", 4"],
        &["shl i32 %x", "shl i32 %y"],
    );
}

/// `((x >>u 4) ^ 15) | (y >>u 4)` kept two shifts; the mask goes back through
/// the shift: `((y | (x ^ 240)) >>u 4)`.
#[test]
fn a_constant_mask_under_a_logical_shift_is_shifted_back() {
    check(
        "i32 %x, i32 %y",
        "  %a = lshr i32 %x, 4\n  %b = xor i32 %a, 15\n  %c = lshr i32 %y, 4\n  %r = or i32 %b, %c\n",
        &["xor i32 %x, 240", "lshr i32 %", ", 4"],
        &["lshr i32 %x", "lshr i32 %y"],
    );
}

/// `(~(x >>s s)) & (y >>s s)` is `(y & ~x) >>s s`.
#[test]
fn an_and_of_a_not_and_an_arithmetic_shift_shifts_the_and() {
    check(
        "i32 %x, i32 %y, i32 %s",
        "  %a = ashr i32 %x, %s\n  %n = xor i32 %a, -1\n  %c = ashr i32 %y, %s\n  %r = and i32 %n, %c\n",
        &["xor i32 %x, -1", "ashr i32 %"],
        &["ashr i32 %x", "ashr i32 %y"],
    );
}

/// `add` distributes over `shl`: `((x << 3) + m) + (y << 3)` is
/// `((x + y) << 3) + m`.
#[test]
fn an_add_of_two_left_shifts_shifts_the_add() {
    check(
        "i32 %x, i32 %y, i32 %m",
        "  %a = shl i32 %x, 3\n  %b = add i32 %a, %m\n  %c = shl i32 %y, 3\n  %r = add i32 %b, %c\n",
        &["add i32 %x, %y", "shl i32 %", ", 3"],
        &["shl i32 %x", "shl i32 %y"],
    );
}

/// `add` does not distribute over `lshr` and `7` is no mask a shift back
/// returns: the pin leaves `((x >>u 3) + 7) + (y >>u 3)` alone.
#[test]
fn an_add_over_a_logical_shift_with_a_lost_mask_stays() {
    check(
        "i32 %x, i32 %y",
        "  %a = lshr i32 %x, 3\n  %b = add i32 %a, 7\n  %c = lshr i32 %y, 3\n  %r = add i32 %b, %c\n",
        &["lshr i32 %x, 3", "lshr i32 %y, 3"],
        &[],
    );
}

/// A runtime routine that returns flags (`B$SCMP`, `runtime.toml`) may be read
/// only by compares: isel refuses it otherwise. The folds of this file, with
/// the other three visits that call it, leave every user of such a call a
/// compare for each predicate, constant and consumer of the compare below.
#[test]
fn test_the_fold_gives_a_flags_result_call_no_user_but_compares() {
    use llrm_mir::opcode::Opcode as Op;
    let consumers: [&str; 8] = [
        "  %r = sext i1 %c to i16\n  ret i16 %r\n",
        "  %r = zext i1 %c to i16\n  ret i16 %r\n",
        "  %r = select i1 %c, i16 -1, i16 0\n  ret i16 %r\n",
        "  %r = select i1 %c, i16 5, i16 7\n  ret i16 %r\n",
        "  %n = xor i1 %c, true\n  %r = zext i1 %n to i16\n  ret i16 %r\n",
        "  %d = icmp sgt i16 %s, 3\n  %e = and i1 %c, %d\n  %r = zext i1 %e to i16\n  ret i16 %r\n",
        "  %d = icmp slt i16 %s, -3\n  %e = or i1 %c, %d\n  %r = zext i1 %e to i16\n  ret i16 %r\n",
        "  %d = icmp ne i16 %s, 0\n  %e = and i1 %c, %d\n  %r = sext i1 %e to i16\n  ret i16 %r\n",
    ];
    for predicate in ["slt", "sle", "sgt", "sge", "eq", "ne", "ult", "ule", "ugt", "uge"] {
        for constant in ["-1", "0", "1"] {
            for tail in consumers {
                let text = format!(
                    "declare i16 @llrm.qb.B$SCMP(ptr, ptr) flagsresult
define i16 @f(ptr %a, ptr %b) {{
b0:
  %s = call i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  %c = icmp {predicate} i16 %s, {constant}
{tail}}}
"
                );
                let mut module = parsed(&text);
                let globals = module.globals.clone();
                let metadata = module.metadata.clone();
                let (context, function) = module.function_mut("f").expect("@f");
                combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
                let call = function
                    .walk()
                    .map(|(_, one)| one)
                    .find(|&one| matches!(function.instruction(one).opcode, Op::Call(_)))
                    .expect("the call");
                let value = function.instruction(call).result.expect("a value");
                assert!(
                    function
                        .users(value)
                        .iter()
                        .all(|one| matches!(function.instruction(one.user).opcode, Op::ICmp(_))),
                    "{predicate} {constant}: a user of the flags that is no compare\n{}",
                    printed(&module)
                );
            }
        }
    }
}
