// SPDX-License-Identifier: GPL-3.0-or-later
//! A reduction wider than a vector register keeps one register of lanes: gcc's
//! single accumulator, `vectorizable_reduction` and `vect_transform_reduction`
//! of
//! [`tree-vect-loop.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/tree-vect-loop.cc)
//! (`single_defuse_cycle`: the copies of one scalar statement chain through one
//! accumulator).
//!
//! # How it works
//! `acc = acc + x` with `x` a `<16 x i64>` is `<2 x i64>` lanes of `acc` and
//! eight pieces of `x` added in turn: the same adds, one live accumulator where
//! LLVM keeps `<16 x i64>` (eight registers, and a byte sum spills). The pieces
//! are the shuffles `x[k * 2 .. k * 2 + 2]`, which the backend takes as the
//! registers `x` is already split into.
//!
//! # DEVIATE rows
//! - LVA1 one accumulator per interleave part (gcc's one per reduction): the
//!   parts' chains stay independent, as the pin's. Exit: none.
//! - LVA2 only an update `acc op x` whose `acc` is the reduction's own phi and
//!   `x` a widened value; a chain of several operations keeps the wide
//!   accumulator.
//!
//! # Runs
//! Inside `loopvectorize`, on a reduction whose vector is wider than a
//! register.
//!
//! # Example
//! `acc = acc + x` with `x: <16 x i64>` on a 128-bit target becomes eight adds
//! of `<2 x i64>` pieces of `x` into one `<2 x i64>` accumulator.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::types::{Type, TypeId};

/// The lanes of the accumulator when a reduction of `vf` lanes of `lane_bits`
/// is wider than a `register_bits` register: the lanes of one register.
pub(crate) fn lanes(
    register_bits: u32,
    lane_bits: u32,
    vf: u32,
) -> Option<u32> {
    let per_register = register_bits.checked_div(lane_bits).filter(|&one| one > 0)?;
    (vf > per_register && vf % per_register == 0).then_some(per_register)
}

/// `acc op x[0..lanes]`, then `op x[lanes..2 * lanes]` and so on: the value of
/// the accumulator after the trip.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fold_pieces(
    context: &mut Context,
    function: &mut Function,
    at: InstId,
    op: BinaryOp,
    element: TypeId,
    lanes: u32,
    vf: u32,
    acc: ValueId,
    wide: ValueId,
) -> ValueId {
    let narrow = context.types.intern(Type::Vector { element, count: lanes });
    let poison =
        Operand::Constant(context.constant(Constant { ty: function.value(wide).ty, kind: ConstantKind::Poison }));
    let mut total = acc;
    for piece in 0..vf / lanes {
        let mask = (piece * lanes..(piece + 1) * lanes).map(|lane| lane as i32).collect();
        let shuffled = function.create_instruction(
            Opcode::ShuffleVector(mask),
            narrow,
            vec![Operand::Value(wide), poison],
            Flags::default(),
            Some("rdx.piece"),
        );
        function.insert(shuffled, Position::Before(at)).expect("the operation's block");
        let piece_value = function.instruction(shuffled).result.expect("a shufflevector");
        let added = function.create_instruction(
            Opcode::Binary(op),
            narrow,
            vec![Operand::Value(total), Operand::Value(piece_value)],
            Flags::default(),
            Some("rdx.acc"),
        );
        function.insert(added, Position::Before(at)).expect("the operation's block");
        total = function.instruction(added).result.expect("a binary operation");
    }
    total
}
