//! A select between two constants made arithmetic on its condition.
//! Translated from `DAGCombiner::foldSelectOfConstants` and
//! `foldSelectOfConstantsUsingSra`
//! ([DAGCombiner.cpp#L13011](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13011)),
//! with X86's `convertSelectOfConstantsToMath` as the target's answer.
//!
//! # How it works
//! `select c, C1, C2` of an integer wider than one bit is, by the constants:
//!
//! ```text
//!  1, 0           zext c                    -1, 0          sext c
//!  0, 1           zext (c ^ 1)              0, -1          sext (c ^ 1)
//!  C1, C1-1       (zext c) + C1-1           C1, C1+1       (sext c) + C1+1
//!  2^k, 0         (zext c) << k             -1, C          (sext c) | C
//!  C, -1          (sext (c ^ 1)) | C
//!  C1, -1         (x >>s w-1) | C1   when c is  x > -1
//!  C1, 0          (x >>s w-1) & C1   when c is  x < 0
//! ```
//!
//! The first four need no target answer; the arithmetic ones ask the target
//! (`Tti::convert_select_of_constants_to_math`), and X86 says yes for every
//! scalar integer. The sign-bit forms apply only when the compare has no other
//! reader and compares a value of the select's own type. What no row covers
//! stays a select, which `selects` turns into a branch where the target has no
//! conditional move.
//!
//! A select of one-bit values is `foldBoolSelectToLogic` (`boolean`): `select
//! c, c|1, f` is `c | f`, `select c, t, c|0` is `c & t`, `select c, t, 1` is
//! `!c | t`, `select c, 0, f` is `!c & f`. The pin freezes the other operand,
//! which no machine value needs, so it is left out.
//!
//! # Example
//! `%r = select i1 %less, i32 4, i32 0` is `%z = zext i1 %less to i32` and
//! `%r = shl i32 %z, 2`: at m32 i486 `setl al; shl eax, 2`, no branch.
//!
//! # Runs
//! Once per program in `driver::optimized`, in front of `selects::lowered`
//! where the target has no conditional move, and at every -O level.

use llrm_mir::context::{ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::TypeId;

/// What the select becomes: arithmetic on its condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Recipe {
    /// `zext c`.
    Zext,
    /// `sext c`.
    Sext,
    /// `zext (c ^ 1)`.
    ZextNot,
    /// `sext (c ^ 1)`.
    SextNot,
    /// `(zext c) + n`.
    AddZext(i128),
    /// `(sext c) + n`.
    AddSext(i128),
    /// `(zext c) << k`.
    ShlZext(u32),
    /// `(sext c) | n`.
    OrSext(i128),
    /// `(sext (c ^ 1)) | n`.
    OrSextNot(i128),
}

/// `foldSelectOfConstants` for `select c, yes, no` of a `width`-bit integer:
/// the recipe, if any. `math` is the target's answer to
/// `convertSelectOfConstantsToMath`.
pub fn recipe(
    yes: i128,
    no: i128,
    width: u32,
    math: bool,
) -> Option<Recipe> {
    if width < 2 {
        return None;
    }
    let (yes, no) = (signed(yes as u128 & mask(width), width), signed(no as u128 & mask(width), width));
    let wraps = |value: i128| signed(value as u128 & mask(width), width);
    match (yes, no) {
        (1, 0) => return Some(Recipe::Zext),
        (-1, 0) => return Some(Recipe::Sext),
        (0, 1) => return Some(Recipe::ZextNot),
        (0, -1) => return Some(Recipe::SextNot),
        _ => {}
    }
    if !math {
        return None;
    }
    if wraps(yes - 1) == no {
        return Some(Recipe::AddZext(no));
    }
    if wraps(yes + 1) == no {
        return Some(Recipe::AddSext(no));
    }
    if no == 0 && (yes as u128 & mask(width)).is_power_of_two() {
        return Some(Recipe::ShlZext((yes as u128 & mask(width)).trailing_zeros()));
    }
    if yes == -1 {
        return Some(Recipe::OrSext(no));
    }
    if no == -1 {
        return Some(Recipe::OrSextNot(yes));
    }
    None
}

fn mask(width: u32) -> u128 {
    llrm_mir::context::mask(width)
}

/// An integer constant operand's signed value.
pub(crate) fn constant(
    context: &Context,
    operand: Operand,
) -> Option<i128> {
    let Operand::Constant(id) = operand else { return None };
    let width = context.types.int_bits(context.get(id).ty)?;
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(signed(bits, width)),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// `select` replaced by arithmetic on its condition, where
/// `foldSelectOfConstants` finds some. Whether it was.
pub fn folded(
    context: &Context,
    function: &mut Function,
    select: InstId,
    math: bool,
) -> Result<bool, String> {
    let op = function.instruction(select).clone();
    let [condition, yes, no] = op.operands[..] else { return Ok(false) };
    let Some(width) = context.types.int_bits(op.ty) else { return Ok(false) };
    let (Some(yes), Some(no)) = (constant(context, yes), constant(context, no)) else { return Ok(false) };
    let result = op.result.ok_or("a select's value")?;
    let made = match recipe(yes, no, width, math) {
        Some(recipe) => built(context, function, select, condition, op.ty, recipe)?,
        None => match sign_test(context, function, condition, op.ty, yes, no, width) {
            Some((value, how)) => sign_mask(context, function, select, op.ty, value, how, yes, width)?,
            None => return Ok(false),
        },
    };
    function.replace_all_uses_with(result, made);
    function.erase(select)?;
    Ok(true)
}

/// A new instruction before `select`, and its value.
pub(crate) fn before(
    function: &mut Function,
    select: InstId,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Result<Operand, String> {
    let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(made, Position::Before(select))?;
    Ok(Operand::Value(function.instruction(made).result.expect("a value")))
}

/// `foldBoolSelectToLogic`: a select of one-bit values with a constant arm, or
/// one arm the condition itself, made `and`/`or` of the condition. Whether it
/// replaced the select.
pub fn boolean(
    context: &Context,
    function: &mut Function,
    select: InstId,
) -> Result<bool, String> {
    let op = function.instruction(select).clone();
    let [condition, yes, no] = op.operands[..] else { return Ok(false) };
    if context.types.int_bits(op.ty) != Some(1) || function.operand_type(context, condition) != Some(op.ty) {
        return Ok(false);
    }
    let result = op.result.ok_or("a select's value")?;
    let (yes_value, no_value) = (constant(context, yes), constant(context, no));
    let not = |function: &mut Function| {
        let one = Operand::Constant(context.int(op.ty, 1));
        before(function, select, Opcode::Binary(BinaryOp::Xor), op.ty, vec![condition, one])
    };
    let made = if yes == condition || yes_value == Some(-1) {
        before(function, select, Opcode::Binary(BinaryOp::Or), op.ty, vec![condition, no])?
    } else if no == condition || no_value == Some(0) {
        before(function, select, Opcode::Binary(BinaryOp::And), op.ty, vec![condition, yes])?
    } else if no_value == Some(-1) {
        let flipped = not(function)?;
        before(function, select, Opcode::Binary(BinaryOp::Or), op.ty, vec![flipped, yes])?
    } else if yes_value == Some(0) {
        let flipped = not(function)?;
        before(function, select, Opcode::Binary(BinaryOp::And), op.ty, vec![flipped, no])?
    } else {
        return Ok(false);
    };
    function.replace_all_uses_with(result, made);
    function.erase(select)?;
    Ok(true)
}

fn built(
    context: &Context,
    function: &mut Function,
    select: InstId,
    condition: Operand,
    ty: TypeId,
    recipe: Recipe,
) -> Result<Operand, String> {
    let bit = function.operand_type(context, condition).ok_or("a typed condition")?;
    let one = Operand::Constant(context.int(bit, 1));
    let wide = |value: i128| Operand::Constant(context.int(ty, value));
    let negated =
        |function: &mut Function| before(function, select, Opcode::Binary(BinaryOp::Xor), bit, vec![condition, one]);
    let extended =
        |function: &mut Function, from: Operand, op: CastOp| before(function, select, Opcode::Cast(op), ty, vec![from]);
    let arithmetic = |function: &mut Function, op: BinaryOp, left: Operand, right: Operand| {
        before(function, select, Opcode::Binary(op), ty, vec![left, right])
    };
    Ok(match recipe {
        Recipe::Zext => extended(function, condition, CastOp::ZExt)?,
        Recipe::Sext => extended(function, condition, CastOp::SExt)?,
        Recipe::ZextNot => {
            let not = negated(function)?;
            extended(function, not, CastOp::ZExt)?
        }
        Recipe::SextNot => {
            let not = negated(function)?;
            extended(function, not, CastOp::SExt)?
        }
        Recipe::AddZext(n) => {
            let flag = extended(function, condition, CastOp::ZExt)?;
            arithmetic(function, BinaryOp::Add, flag, wide(n))?
        }
        Recipe::AddSext(n) => {
            let flag = extended(function, condition, CastOp::SExt)?;
            arithmetic(function, BinaryOp::Add, flag, wide(n))?
        }
        Recipe::ShlZext(k) => {
            let flag = extended(function, condition, CastOp::ZExt)?;
            arithmetic(function, BinaryOp::Shl, flag, wide(i128::from(k)))?
        }
        Recipe::OrSext(n) => {
            let flag = extended(function, condition, CastOp::SExt)?;
            arithmetic(function, BinaryOp::Or, flag, wide(n))?
        }
        Recipe::OrSextNot(n) => {
            let not = negated(function)?;
            let flag = extended(function, not, CastOp::SExt)?;
            arithmetic(function, BinaryOp::Or, flag, wide(n))?
        }
    })
}

/// The compare a value is, where nothing else reads it and it sits in `block`.
fn lone_compare(
    function: &Function,
    operand: Operand,
    block: Option<llrm_mir::module::BlockId>,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def else { return None };
    (function.users(value).has_one_use() && function.parent(inst) == block).then_some(inst)
}

/// `add (sext (icmp p a b)), C` made `add (zext (icmp !p a b)), C-1` for a
/// nonzero `C`, the sign extension and the compare read once and in the add's
/// block: `visitADDLikeCommutative` makes the sum `sub C, (zext c)` and X86's
/// `combineSubSetcc` the inverted `setcc` plus `C-1`, an `inc` or a
/// displacement where `C-1` is 1 (bintree's `select c, 1, 2`: `setge`,
/// `[edi+ebx*4+4]` where `setl; neg; shl` was). How many it rewrote.
pub fn inverted_setcc_sums(
    context: &Context,
    function: &mut Function,
) -> Result<usize, String> {
    let adds: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::Add))
        .collect();
    let mut made = 0;
    for add in adds {
        let op = function.instruction(add).clone();
        let Some(width) = context.types.int_bits(op.ty) else { continue };
        let [left, right] = op.operands[..] else { continue };
        let (extended, sum) = match (constant(context, left), constant(context, right)) {
            (None, Some(sum)) => (left, sum),
            (Some(sum), None) => (right, sum),
            _ => continue,
        };
        if signed(sum as u128 & mask(width), width) == 0 {
            continue;
        }
        let block = function.parent(add);
        let Some(extension) = lone_compare(function, extended, block) else { continue };
        let ext = function.instruction(extension).clone();
        if ext.opcode != Opcode::Cast(CastOp::SExt) {
            continue;
        }
        let [condition] = ext.operands[..] else { continue };
        let Some(compare) = lone_compare(function, condition, block) else { continue };
        let cmp = function.instruction(compare).clone();
        let Opcode::ICmp(predicate) = cmp.opcode else { continue };
        let inverted = function.create_instruction(
            Opcode::ICmp(predicate.inverse()),
            cmp.ty,
            cmp.operands.clone(),
            Flags::default(),
            None,
        );
        function.insert(inverted, Position::Before(add))?;
        let flag = Operand::Value(function.instruction(inverted).result.ok_or("a compare's value")?);
        let zero = before(function, add, Opcode::Cast(CastOp::ZExt), op.ty, vec![flag])?;
        let less = signed((sum - 1) as u128 & mask(width), width);
        let result = if less == 0 {
            zero
        } else {
            let less = Operand::Constant(context.int(op.ty, less));
            before(function, add, Opcode::Binary(BinaryOp::Add), op.ty, vec![zero, less])?
        };
        function.replace_all_uses_with(op.result.ok_or("an add's value")?, result);
        function.erase(add)?;
        function.erase(extension)?;
        function.erase(compare)?;
        made += 1;
    }
    Ok(made)
}

/// How a sign test selects: its value smeared into a mask and combined.
#[derive(Clone, Copy)]
enum Smear {
    /// `x < 0 ? C1 : 0`: `(x >>s w-1) & C1`.
    And,
    /// `x > -1 ? C1 : -1`: `(x >>s w-1) | C1`.
    Or,
}

/// `foldSelectOfConstantsUsingSra`: the tested value and how, where the
/// condition is a sign test of a value of the select's type and nothing else
/// reads it.
fn sign_test(
    context: &Context,
    function: &Function,
    condition: Operand,
    ty: TypeId,
    _yes: i128,
    no: i128,
    width: u32,
) -> Option<(Operand, Smear)> {
    let Operand::Value(value) = condition else { return None };
    let llrm_mir::module::ValueDef::Instruction(compare) = function.value(value).def else { return None };
    if function.users(value).only().is_none() {
        return None;
    }
    let instruction = function.instruction(compare);
    let Opcode::ICmp(predicate) = instruction.opcode else { return None };
    let [tested, against] = instruction.operands[..] else { return None };
    if function.operand_type(context, tested) != Some(ty) {
        return None;
    }
    let against = constant(context, against)?;
    let no = signed(no as u128 & mask(width), width);
    match predicate {
        IntPredicate::Sgt if against == -1 && no == -1 => Some((tested, Smear::Or)),
        IntPredicate::Slt if against == 0 && no == 0 => Some((tested, Smear::And)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn sign_mask(
    context: &Context,
    function: &mut Function,
    select: InstId,
    ty: TypeId,
    tested: Operand,
    how: Smear,
    yes: i128,
    width: u32,
) -> Result<Operand, String> {
    let shift = Operand::Constant(context.int(ty, i128::from(width - 1)));
    let mask = before(function, select, Opcode::Binary(BinaryOp::AShr), ty, vec![tested, shift])?;
    let value = Operand::Constant(context.int(ty, yes));
    let op = match how {
        Smear::And => BinaryOp::And,
        Smear::Or => BinaryOp::Or,
    };
    before(function, select, Opcode::Binary(op), ty, vec![mask, value])
}

#[cfg(test)]
mod tests {
    use llrm_mir::interpret::{Val, run};

    use super::{Recipe, recipe};

    /// The forms clang -O2 -march=i486 gives `a < b ? 4 : 0` (`setl; shl`),
    /// `? 5 : 4` (`setl; or`/`add`), `? 1 : 0` (`setl`) and `? -1 : 0`
    /// (`setl; neg`): each a recipe, where a branch around a move was made.
    #[test]
    fn test_the_constants_of_a_select_choose_their_arithmetic() {
        assert_eq!(recipe(4, 0, 32, true), Some(Recipe::ShlZext(2)));
        assert_eq!(recipe(5, 4, 32, true), Some(Recipe::AddZext(4)));
        assert_eq!(recipe(1, 0, 32, false), Some(Recipe::Zext));
        assert_eq!(recipe(-1, 0, 32, false), Some(Recipe::Sext));
        assert_eq!(recipe(0, 1, 16, false), Some(Recipe::ZextNot));
        assert_eq!(recipe(0, -1, 16, false), Some(Recipe::SextNot));
        assert_eq!(recipe(4, 5, 32, true), Some(Recipe::AddSext(5)));
        assert_eq!(recipe(-1, 7, 32, true), Some(Recipe::OrSext(7)));
        assert_eq!(recipe(7, -1, 32, true), Some(Recipe::OrSextNot(7)));
        // The arithmetic forms ask the target; the first four do not.
        assert_eq!(recipe(4, 0, 32, false), None);
        assert_eq!(recipe(1, 0, 32, false), Some(Recipe::Zext));
        // No form: two constants that differ by 3, neither a power of two over
        // 0.
        assert_eq!(recipe(10, 7, 32, true), None);
        // One bit wide: left to the logic fold.
        assert_eq!(recipe(1, 0, 1, true), None);
    }

    /// bintree's `insert`: `select (a < b), 1, 2` folds to `(sext c) + 2`,
    /// which was `setl; movzx; neg; shl 2` before the address. The pin's
    /// `combineSubSetcc` makes it `zext (a >= b) + 1` (`setge`, the 1 a
    /// displacement): bintree m32 -O1 93475 -> 90983 clocks. A compare read
    /// twice, or a sum of zero, stays.
    #[test]
    fn test_a_sign_extended_compare_plus_a_constant_is_the_inverse_compare_plus_one_less() {
        let text = "define i32 @f(i32 %a, i32 %b) {
entry:
  %c = icmp slt i32 %a, %b
  %s = sext i1 %c to i32
  %r = add i32 %s, 2
  %d = icmp eq i32 %a, %b
  %t = sext i1 %d to i32
  %u = add i32 %t, 0
  %v = add i32 %r, %u
  ret i32 %v
}
";
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        let llrm_mir::module::GlobalKind::Function(function) = &mut after.globals[0].kind else { panic!("a function") };
        assert_eq!(super::inverted_setcc_sums(&after.context, function), Ok(1));
        let printed = llrm_mir::print::module(&after);
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{printed}");
        assert!(printed.contains("icmp sge i32 %a, %b") && !printed.contains("icmp slt"), "{printed}");
        assert_eq!(printed.matches("sext").count(), 1, "{printed}");
        for (a, b) in [(1, 2), (2, 1), (5, 5), (-3, 4)] {
            let arguments = vec![
                Val::Int { bits: a as u128 & 0xFFFF_FFFF, width: 32 },
                Val::Int { bits: b as u128 & 0xFFFF_FFFF, width: 32 },
            ];
            assert_eq!(run(&after, "f", arguments.clone(), 100).unwrap(), run(&before, "f", arguments, 100).unwrap());
        }
    }

    /// The difference wraps at the type's width: `i8` -128 and 127 differ by 1.
    #[test]
    fn test_constants_that_differ_by_one_around_the_width_make_a_sum() {
        assert_eq!(recipe(-128, 127, 8, true), Some(Recipe::AddZext(127)));
        assert_eq!(recipe(0x80, 0, 8, true), Some(Recipe::ShlZext(7)));
    }
}
