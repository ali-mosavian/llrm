// SPDX-License-Identifier: GPL-3.0-or-later
//! Lowers the vectors one machine word holds to that integer. Translated from
//! gcc's `tree-vect-generic.cc` (`expand_vector_parallel`,
//! `expand_vector_piecewise` over a word type, `lower_vec_perm` of a splat),
//! the pass that makes `ix86_preferred_simd_mode`'s word-mode vectors of a
//! target with no SSE code.
//!
//! # How it works
//! A `<2 x i16>` or `<4 x i8>` value is 32 bits, the word's width. Where the
//! target says so (`Tti::word_vectors`) each such value becomes an `i32` whose
//! lane `k` is bits `k*L .. (k+1)*L` (the memory order of a little-endian
//! target, so a vector load is the word load):
//!
//! ```text
//!   %v = load <2 x i16>, ptr %p          %v = load i32, ptr %p
//!   %w = xor <2 x i16> %v, %s            %w = xor i32 %v, %s
//!   store <2 x i16> %w, ptr %q           store i32 %w, ptr %q
//!   %i = insertelement poison, i16 %x, 0 %i = zext i16 %x to i32
//!   %s = shufflevector %i, poison, <0,0> %s = or (%i, shl %i, 16)
//! ```
//!
//! Lanes of a bitwise operation do not interact, so `and`, `or`, `xor`, `phi`,
//! `select` and `freeze` keep their opcode at the word's type. Loads and
//! stores keep their address and alignment. Lane inserts, extracts and
//! shuffles are shifts, masks and `or`s; a splat of one lane doubles it.
//!
//! The pass is all or nothing for a function: one use of a word vector it does
//! not lower (an add, a compare, a call argument) leaves the function as it
//! is, so the selector refuses it as it did.
//!
//! # Runs
//! In `cgpaddr`, with the other CodeGenPrepare rewrites, where the target's
//! `Tti::word_vectors` is true.
//!
//! # See also
//! `loopvectorize`, which makes the vectors where the target prices them
//! (`Tti::vector_arithmetic_cost`: only the bitwise operations).

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::{Type, TypeId};

/// A vector of `count` lanes of `bits` bits that fills a 32-bit word.
#[derive(Clone, Copy)]
struct Word {
    bits: u32,
    count: u32,
}

impl Word {
    fn mask(self) -> u128 {
        (1u128 << self.bits) - 1
    }
}

fn word_of(
    context: &Context,
    ty: TypeId,
) -> Option<Word> {
    let Type::Vector { element, count } = context.types.get(ty) else { return None };
    let bits = context.types.int_bits(*element)?;
    (matches!(bits, 8 | 16) && bits * count == 32).then_some(Word { bits, count: *count })
}

fn type_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<TypeId> {
    function.operand_type(context, operand)
}

/// The integer a constant word vector is, or a whole `undef`/`poison` of it.
fn constant_word(
    context: &Context,
    word: Word,
    id: llrm_mir::context::ConstantId,
) -> Option<Constant> {
    let i32_ty = context.types.int(32);
    let kind = match &context.get(id).kind {
        ConstantKind::Zero => ConstantKind::Int(0),
        ConstantKind::Undef => ConstantKind::Undef,
        ConstantKind::Poison => ConstantKind::Poison,
        ConstantKind::Aggregate(lanes) => {
            let mut bits = 0u128;
            for (index, lane) in lanes.iter().enumerate() {
                let value = match &context.get(*lane).kind {
                    ConstantKind::Int(value) => *value & word.mask(),
                    ConstantKind::Zero | ConstantKind::Null | ConstantKind::Undef | ConstantKind::Poison => 0,
                    _ => return None,
                };
                bits |= value << (index as u32 * word.bits);
            }
            ConstantKind::Int(bits)
        }
        _ => return None,
    };
    Some(Constant { ty: i32_ty, kind })
}

fn is_poison_or_undef(
    context: &Context,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Undef | ConstantKind::Poison)
    )
}

fn constant_index(
    context: &Context,
    operand: Operand,
) -> Option<u32> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(value) => u32::try_from(value).ok(),
            ConstantKind::Zero => Some(0),
            _ => None,
        },
        _ => None,
    }
}

/// Whether every instruction that touches a word vector is one this pass
/// lowers.
fn supported(
    context: &Context,
    function: &Function,
    work: &[InstId],
) -> bool {
    for &inst in work {
        let one = function.instruction(inst);
        let result_word = word_of(context, one.ty);
        let ok = match &one.opcode {
            Opcode::Load { .. } | Opcode::Freeze => result_word.is_some(),
            Opcode::Store { .. } => true,
            Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) | Opcode::Phi => result_word.is_some(),
            Opcode::Select => {
                result_word.is_some()
                    && type_of(context, function, one.operands[0])
                        .is_some_and(|ty| !matches!(context.types.get(ty), Type::Vector { .. }))
            }
            Opcode::InsertElement => result_word.is_some() && constant_index(context, one.operands[2]).is_some(),
            Opcode::ExtractElement => constant_index(context, one.operands[1]).is_some(),
            Opcode::ShuffleVector(_) => {
                result_word.is_some()
                    && one.operands.iter().all(|&operand| {
                        type_of(context, function, operand).and_then(|ty| word_of(context, ty)).is_some()
                    })
            }
            Opcode::Cast(CastOp::BitCast) => {
                let from = type_of(context, function, one.operands[0]);
                let word_in = from.and_then(|ty| word_of(context, ty)).is_some();
                let word_out = result_word.is_some();
                let int32 = |ty: Option<TypeId>| ty.is_some_and(|ty| context.types.int_bits(ty) == Some(32));
                (word_in && int32(Some(one.ty))) || (word_out && int32(from)) || (word_in && word_out)
            }
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

/// Places a new instruction before `anchor`, and gives its value.
fn emit(
    function: &mut Function,
    anchor: InstId,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Operand {
    let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(made, Position::Before(anchor)).expect("a placed anchor");
    Operand::Value(function.instruction(made).result.expect("a value"))
}

/// Every vector the word holds in `function`, as the integer of its width.
/// Whether anything changed.
pub fn lowered(
    context: &Context,
    function: &mut Function,
) -> bool {
    let insts: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let touches = |function: &Function, inst: InstId| {
        let one = function.instruction(inst);
        word_of(context, one.ty).is_some()
            || one.operands.iter().any(|&operand| {
                !matches!(operand, Operand::Block(_))
                    && type_of(context, function, operand).and_then(|ty| word_of(context, ty)).is_some()
            })
    };
    let work: Vec<InstId> = insts.into_iter().filter(|&inst| touches(function, inst)).collect();
    if work.is_empty() || !supported(context, function, &work) {
        return false;
    }
    let i32_ty = context.types.int(32);
    let konst = |value: u128| Operand::Constant(context.int(i32_ty, value as i128));
    // the scalar a lane-0 insert into poison put, by the insert's old value
    let mut clean: IdMap<ValueId, (Operand, u32)> = IdMap::new();
    for inst in work {
        // the word this instruction works in, from the types it has now
        let before = function.instruction(inst).clone();
        let word = word_of(context, before.ty).or_else(|| {
            before
                .operands
                .iter()
                .filter(|operand| !matches!(operand, Operand::Block(_)))
                .find_map(|&operand| type_of(context, function, operand).and_then(|ty| word_of(context, ty)))
        });
        // the constant vector operands are integers now
        for (at, &operand) in before.operands.iter().enumerate() {
            if let Operand::Constant(id) = operand
                && let Some(word) = word_of(context, context.get(id).ty)
                && let Some(constant) = constant_word(context, word, id)
            {
                function.set_operand(inst, at, Operand::Constant(context.constant(constant)));
            }
        }
        let one = function.instruction(inst).clone();
        let operands = one.operands.clone();
        let result = one.result;
        // An extract reads its lane width from its own result: where an earlier
        // shuffle or insert already made its vector an `i32`, no operand says
        // it is a vector any more, and the extract stayed (an
        // `extractelement` of an `i32` that selection refuses).
        let word = match (&one.opcode, word) {
            (Opcode::ExtractElement, None) => context
                .types
                .int_bits(one.ty)
                .filter(|&bits| bits > 0 && bits < 32)
                .map(|bits| Word { bits, count: 32 / bits }),
            (_, word) => word,
        };
        let Some(word) = word else { continue };
        match &one.opcode {
            Opcode::Load { .. } | Opcode::Freeze | Opcode::Binary(_) | Opcode::Phi | Opcode::Select => {
                function.set_type(inst, i32_ty)
            }
            Opcode::Store { .. } => {}
            Opcode::Cast(CastOp::BitCast) => {
                let from = operands[0];
                let value = result.expect("a bitcast has a value");
                function.replace_all_uses_with(value, from);
                function.erase(inst).expect("a bitcast nothing reads");
            }
            Opcode::InsertElement => {
                let lane = constant_index(context, operands[2]).expect("a constant lane");
                let shift = lane * word.bits;
                let elt_ty = function.operand_type(context, operands[1]).expect("a typed lane");
                let element = match operands[1] {
                    Operand::Constant(id) => match context.get(id).kind {
                        ConstantKind::Int(value) => konst(value & word.mask()),
                        _ => konst(0),
                    },
                    other => emit(function, inst, Opcode::Cast(CastOp::ZExt), i32_ty, vec![other]),
                };
                let _ = elt_ty;
                let placed = if shift == 0 {
                    element
                } else {
                    emit(function, inst, Opcode::Binary(BinaryOp::Shl), i32_ty, vec![element, konst(u128::from(shift))])
                };
                let value = result.expect("an insert has a value");
                let whole = if is_poison_or_undef(context, operands[0]) {
                    clean.insert(value, (element, lane));
                    placed
                } else {
                    let keep = !(word.mask() << shift) & u128::from(u32::MAX);
                    let rest =
                        emit(function, inst, Opcode::Binary(BinaryOp::And), i32_ty, vec![operands[0], konst(keep)]);
                    emit(function, inst, Opcode::Binary(BinaryOp::Or), i32_ty, vec![rest, placed])
                };
                function.replace_all_uses_with(value, whole);
                function.erase(inst).expect("an insert nothing reads");
            }
            Opcode::ExtractElement => {
                let lane = constant_index(context, operands[1]).expect("a constant lane");
                let bits_ty = context.types.int(word.bits);
                let shifted = if lane == 0 {
                    operands[0]
                } else {
                    emit(
                        function,
                        inst,
                        Opcode::Binary(BinaryOp::LShr),
                        i32_ty,
                        vec![operands[0], konst(u128::from(lane * word.bits))],
                    )
                };
                let value = result.expect("an extract has a value");
                let lane_value = emit(function, inst, Opcode::Cast(CastOp::Trunc), bits_ty, vec![shifted]);
                function.replace_all_uses_with(value, lane_value);
                function.erase(inst).expect("an extract nothing reads");
            }
            Opcode::ShuffleVector(mask) => {
                let value = result.expect("a shuffle has a value");
                let whole = shuffled(context, function, inst, word, mask, &operands, &clean, i32_ty);
                function.replace_all_uses_with(value, whole);
                function.erase(inst).expect("a shuffle nothing reads");
            }
            _ => unreachable!("`supported` admitted it"),
        }
    }
    true
}

/// The word a shuffle of two words is: each result lane is a lane of one of
/// the inputs (a splat of one lane doubles it).
#[allow(clippy::too_many_arguments)]
fn shuffled(
    context: &Context,
    function: &mut Function,
    inst: InstId,
    word: Word,
    mask: &[i32],
    operands: &[Operand],
    clean: &IdMap<ValueId, (Operand, u32)>,
    i32_ty: TypeId,
) -> Operand {
    let konst = |value: u128| Operand::Constant(context.int(i32_ty, value as i128));
    let mut picks = mask.iter().copied().filter(|&pick| pick >= 0);
    let first = picks.next();
    // a splat of one lane of the first input
    if let Some(lane) = first
        && (lane as u32) < word.count
        && mask.iter().all(|&pick| pick < 0 || pick == lane)
        && mask.iter().all(|&pick| pick >= 0)
    {
        let source = match operands[0] {
            Operand::Value(value) if clean.get(&value).is_some_and(|&(_, was)| was == lane as u32) => {
                clean.get(&value).expect("checked").0
            }
            other => {
                let shifted = if lane == 0 {
                    other
                } else {
                    emit(
                        function,
                        inst,
                        Opcode::Binary(BinaryOp::LShr),
                        i32_ty,
                        vec![other, konst(u128::from(lane as u32 * word.bits))],
                    )
                };
                if (lane as u32 + 1) * word.bits == 32 {
                    shifted
                } else {
                    emit(function, inst, Opcode::Binary(BinaryOp::And), i32_ty, vec![shifted, konst(word.mask())])
                }
            }
        };
        let mut one = source;
        let mut width = word.bits;
        while width < 32 {
            let up = emit(function, inst, Opcode::Binary(BinaryOp::Shl), i32_ty, vec![one, konst(u128::from(width))]);
            one = emit(function, inst, Opcode::Binary(BinaryOp::Or), i32_ty, vec![one, up]);
            width *= 2;
        }
        return one;
    }
    // any other: each lane picked, moved to its place, and joined
    let mut joined: Option<Operand> = None;
    for (place, &pick) in mask.iter().enumerate() {
        if pick < 0 {
            continue;
        }
        let (input, lane) = if (pick as u32) < word.count { (0, pick as u32) } else { (1, pick as u32 - word.count) };
        if is_poison_or_undef(context, operands[input]) {
            continue;
        }
        let from = lane * word.bits;
        let to = place as u32 * word.bits;
        let mut part = operands[input];
        if from > to {
            part =
                emit(function, inst, Opcode::Binary(BinaryOp::LShr), i32_ty, vec![part, konst(u128::from(from - to))]);
        } else if to > from {
            part =
                emit(function, inst, Opcode::Binary(BinaryOp::Shl), i32_ty, vec![part, konst(u128::from(to - from))]);
        }
        part = emit(function, inst, Opcode::Binary(BinaryOp::And), i32_ty, vec![part, konst(word.mask() << to)]);
        joined = Some(match joined {
            Some(so_far) => emit(function, inst, Opcode::Binary(BinaryOp::Or), i32_ty, vec![so_far, part]),
            None => part,
        });
    }
    joined.unwrap_or_else(|| konst(0))
}
