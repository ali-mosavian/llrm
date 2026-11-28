//! Narrow the integer chains a loop computes from its induction.
//!
//! Translation: GCC `vect_determine_precisions` and
//! `vect_recog_over_widening_pattern` (`tree-vect-patterns.cc`): an operation
//! whose result needs fewer bits than its type is done at that width.
//! DEVIATE LV-NARROW-IV (owner llrm-target-x86-m64-b, exit 2026-11-30): the pin
//! has no such step; its `computeMinimumValueSizes` stops at a phi and has no
//! root here, so `(i & 15) + 1` of an `i64` counter is a `<8 x i64>` chain
//! (`pand`, `paddq`, then shuffles to the `i16` multiply). gcc keeps the
//! counter in `<8 x i16>`.
//!
//! `and`, `or`, `xor`, `add` and `mul` of the counter and constants whose
//! result fits 16 or 32 bits become `sext (op (trunc counter), constants)`
//! (`zext` where the result fills the lane): the low bits of those operations
//! depend only on the low bits of their operands, and the result's upper bits
//! are zero. The `trunc` of the counter is what the vectorizer already widens
//! to a narrow `vec.ind`.
//!
//! # Runs
//! Inside `loopvectorize`, on the integer chains of a loop's induction before
//! the plan is costed.
//!
//! # Example
//! `(i & 15) + 1` with `i: i64` feeding a 16-bit multiply becomes
//! `sext ((trunc i to i16) & 15) + 1)`: one `<8 x i16>` chain, not a
//! `<8 x i64>` chain shuffled down to the multiply.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};

/// The narrowest lane the chain is narrowed to: the vector of an `i8` chain
/// would be half a register.
const NARROWEST: u32 = 16;

/// The widest result a chain is narrowed to: an operation whose result fits 16
/// or 32 bits is done at that width (the module's rule); a wider one keeps the
/// counter's type. No pin twin: part of DEVIATE LV-NARROW-IV.
const WIDEST: u32 = 32;

/// The lane a chain of `bits` is narrowed to: not under the narrowest access of
/// the loop, whose vector factor fills the registers with that lane; a narrower
/// chain is a half-register vector and the conversions to the wide lanes.
fn lane_width(
    bits: u32,
    floor: u32,
) -> u32 {
    bits.max(floor).max(NARROWEST).next_power_of_two()
}

/// The width of the narrowest load or store of the block (0 if it has none).
fn narrowest_access(
    context: &Context,
    function: &Function,
    header: BlockId,
) -> u32 {
    function
        .iter_block(header)
        .filter_map(|inst| {
            let data = function.instruction(inst);
            let ty = match data.opcode {
                Opcode::Load { .. } => Some(data.ty),
                Opcode::Store { .. } => function.operand_type(context, data.operands[0]),
                _ => None,
            };
            ty.and_then(|ty| context.types.int_bits(ty))
        })
        .min()
        .unwrap_or(0)
}

/// What each operation of the loop needs, and what it is made of.
struct Member {
    inst: InstId,
    bits: u32,
}

/// The bits a value can have set: a constant's width, a member's computed bits,
/// else its type's.
fn used_bits(
    context: &Context,
    function: &Function,
    members: &IdMap<ValueId, u32>,
    operand: Operand,
) -> u32 {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(value) => 128 - value.leading_zeros(),
            _ => u32::MAX,
        },
        Operand::Value(value) => members.get(&value).copied().unwrap_or_else(|| {
            function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).unwrap_or(u32::MAX)
        }),
        _ => u32::MAX,
    }
}

/// Narrows the chains of `header`'s loop that read `primary_phi`; whether it
/// changed anything.
pub(crate) fn narrow_induction_chains(
    context: &mut Context,
    function: &mut Function,
    header: BlockId,
    primary_phi: Operand,
) -> bool {
    let Operand::Value(phi_value) = primary_phi else { return false };
    let floor = narrowest_access(context, function, header);
    let mut bits_of: IdMap<ValueId, u32> = IdMap::new();
    let mut rooted: IdMap<ValueId, bool> = IdMap::new();
    let mut members: Vec<Member> = Vec::new();
    for inst in function.iter_block(header).collect::<Vec<_>>() {
        let data = function.instruction(inst);
        let (Some(result), Opcode::Binary(op)) = (data.result, data.opcode.clone()) else { continue };
        let Some(wide) = context.types.int_bits(data.ty) else { continue };
        if !matches!(
            op,
            BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Add | BinaryOp::Mul
        )
            || wide <= NARROWEST
        {
            continue;
        }
        let operands = [data.operands[0], data.operands[1]];
        // Made of the counter, constants and other members only: a load or a
        // call in it keeps its width.
        let made_of = |one: Operand| match one {
            Operand::Constant(_) => true,
            Operand::Value(value) => value == phi_value || bits_of.contains_key(&value),
            _ => false,
        };
        if !operands.iter().all(|&one| made_of(one)) {
            continue;
        }
        let reads_counter = operands
            .iter()
            .any(
                |&one| match one {
                    Operand::Value(value) => value == phi_value || rooted.get(&value) == Some(&true),
                    _ => false,
                },
            );
        let [a, b] = operands.map(|one| used_bits(context, function, &bits_of, one));
        let bits = match op {
            BinaryOp::And => a.min(b),
            BinaryOp::Or | BinaryOp::Xor => a.max(b),
            BinaryOp::Add => a.max(b).saturating_add(1),
            _ => a.saturating_add(b),
        }
        .min(wide);
        bits_of.insert(result, bits);
        rooted.insert(result, reads_counter);
        if reads_counter && bits <= WIDEST && lane_width(bits, floor) < wide {
            members.push(Member { inst, bits });
        } else {
            // Not narrowed: its operands may still be members, which keep the
            // wide value.
            bits_of.remove(&result);
            rooted.remove(&result);
        }
    }
    if members.is_empty() {
        return false;
    }

    let first =
        function.iter_block(header).find(|&inst| function.instruction(inst).opcode != Opcode::Phi).expect("a body");
    let mut counters: Vec<(u32, Operand)> = Vec::new();
    let mut narrowed: IdMap<ValueId, (Operand, u32)> = IdMap::new();
    let mut zexts: Vec<InstId> = Vec::new();
    for member in &members {
        let width = lane_width(member.bits, floor);
        let narrow_ty = context.types.int(width);
        let data = function.instruction(member.inst);
        let (op, wide_ty, result, original) =
            (data.opcode.clone(), data.ty, data.result.expect("a result"), [data.operands[0], data.operands[1]]);
        let before = Position::Before(member.inst);
        let mut operands = Vec::with_capacity(2);
        for one in original {
            let made = match one {
                Operand::Constant(id) => match context.get(id).kind {
                    ConstantKind::Int(value) => Operand::Constant(context.int(narrow_ty, value as i128)),
                    _ => unreachable!("a member's constant is an integer"),
                },
                Operand::Value(value) if value == phi_value => match counters.iter().find(|(w, _)| *w == width) {
                    Some(&(_, cut)) => cut,
                    None => {
                        let cut = function.create_instruction(
                            Opcode::Cast(CastOp::Trunc),
                            narrow_ty,
                            vec![primary_phi],
                            Flags::default(),
                            Some("counter.narrow"),
                        );
                        function.insert(cut, Position::Before(first)).expect("the body");
                        let cut = Operand::Value(function.instruction(cut).result.expect("a trunc"));
                        counters.push((width, cut));
                        cut
                    }
                },
                Operand::Value(value) => {
                    let (inner, have) = *narrowed.get(&value).expect("an earlier member");
                    if have == width {
                        inner
                    } else {
                        let cast = if have > width { CastOp::Trunc } else { CastOp::ZExt };
                        let moved = function.create_instruction(
                            Opcode::Cast(cast),
                            narrow_ty,
                            vec![inner],
                            Flags::default(),
                            Some("chain.resize"),
                        );
                        function.insert(moved, before).expect("the body");
                        Operand::Value(function.instruction(moved).result.expect("a cast"))
                    }
                }
                _ => unreachable!("a member's operand is a constant or a value"),
            };
            operands.push(made);
        }
        let narrow = function.create_instruction(op, narrow_ty, operands, Flags::default(), Some("chain.narrow"));
        function.insert(narrow, before).expect("the body");
        let narrow_value = Operand::Value(function.instruction(narrow).result.expect("a result"));
        narrowed.insert(result, (narrow_value, width));
        // A result narrower than its lane has the lane's sign bit clear: `sext`
        // is the `zext`, and the multiply it feeds is the one the
        // target prices and selects for two sign-extended halves.
        let extend = if member.bits < width { CastOp::SExt } else { CastOp::ZExt };
        let widened = function.create_instruction(
            Opcode::Cast(extend),
            wide_ty,
            vec![narrow_value],
            Flags::default(),
            Some("chain.wide"),
        );
        function.insert(widened, before).expect("the body");
        zexts.push(widened);
    }
    // The wide values go: each use reads its `zext`, and the originals, now
    // unused, are erased users first.
    for (member, &widened) in members.iter().zip(&zexts) {
        let result = function.instruction(member.inst).result.expect("a result");
        let with = Operand::Value(function.instruction(widened).result.expect("a zext"));
        function.replace_all_uses_with(result, with);
    }
    for member in members.iter().rev() {
        function.erase(member.inst).expect("an unused original");
    }
    for &widened in zexts.iter().rev() {
        let unused = function.instruction(widened).result.is_some_and(|value| function.users(value).is_empty());
        if unused {
            function.erase(widened).expect("an unused zext");
        }
    }
    true
}

#[cfg(test)]
#[path = "loopvectorizenarrow_tests.rs"]
mod tests;
