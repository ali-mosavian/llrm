//! A masked counter read as `a[i & M]` is pre-scaled to the byte offset.
//!
//! # How it works
//! A loop counter `i` (width `w`, constant step `c`) whose only readers are
//! its own latch add, one `and i, M` and an exit `icmp eq/ne` of `i` or `i + c`
//! against a constant `C`, where the `and` is read only as the index of
//! `gep`s over elements of `S = 2^k` bytes, becomes `i' = S * i` (wrapping):
//! start, step and `C` times `S`, mask `M * S`, and the `gep`s over bytes. The
//! address then needs no scaled index, which real mode forms with a prefix and
//! an extra `shl`.
//!
//! The rewrite is exact when the scaled counter reaches the exit value on the
//! same trip as the original: `S * t == S * C` is `t == C` mod `2^(w-k)`. Both
//! first hits are solved in closed form (an odd step or not) and must agree,
//! which is the trip count staying within the scaled counter's period. `M * S`
//! must stay below `2^(w-1)`, so the byte offset is the same under any
//! extension. The latch add loses its wrap flags. Only a target that prices a
//! scaled index (`Tti::scaled_index_insns`) runs it. Cost: O(uses) per loop.
//!
//! DEVIATE LP-7 (no pin pass: LLVM's LSR has no formula for a masked, scaled
//! counter, and gcc's ivopts (`iv_period`, `iv_elimination_compare`,
//! tree-ssa-loop-ivopts.cc) rewrites an exit test by the counter's period but
//! never the mask; exit: pin LSR gains the formula, this is deleted; owner
//! w-2).
//!
//! # Example
//! The ring loop of c's `ring` at m16 (schematic):
//!
//! ```text
//!  %i = phi i16 [ 3 ], [ %next ]            %i = phi i16 [ 12 ], [ %next ]
//!  %m = and i16 %i, 1023                    %m = and i16 %i, 4092
//!  %p = gep [4 x i8], %buf, %m        ==>   %p = gep i8, %buf, %m
//!  %next = add i16 %i, 5                    %next = add i16 %i, 20
//!  %c = icmp eq i16 %next, 30003            %c = icmp eq i16 %next, 54476
//! ```
//!
//! From the unit test `the_ring_counter_is_the_byte_offset` in
//! `maskscale_tests.rs`.
//!
//! # Runs
//! After `lsr` and `differences`, in the strength slot
//! ([`crate::pipeline::strength_slot`]); option `maskscale`.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

pub struct MaskScale;

/// What was matched: the counter's pieces and the constants it rewrites.
struct Ring {
    phi: InstId,
    start_slot: usize,
    next: InstId,
    and: InstId,
    geps: Vec<InstId>,
    icmp: InstId,
    exit: u128,
    mask: u128,
    step: u128,
    start: u128,
    width: u32,
    shift: u32,
}

fn constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

fn def(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(def) => Some(def),
        _ => None,
    }
}

fn wrap(
    value: u128,
    bits: u32,
) -> u128 {
    if bits >= 128 { value } else { value & ((1u128 << bits) - 1) }
}

/// The least `j` with `c * j == d` mod `2^bits`, if there is one.
fn first_hit(
    c: u128,
    d: u128,
    bits: u32,
) -> Option<u128> {
    let (c, d) = (wrap(c, bits), wrap(d, bits));
    if d == 0 {
        return Some(0);
    }
    if c == 0 {
        return None;
    }
    let low = c.trailing_zeros();
    if d.trailing_zeros() < low {
        return None;
    }
    let rest = bits - low;
    let odd = c >> low;
    // Newton's inverse of an odd number mod 2^128, then cut to `rest` bits.
    let mut inverse = odd;
    for _ in 0..7 {
        inverse = inverse.wrapping_mul(2u128.wrapping_sub(odd.wrapping_mul(inverse)));
    }
    Some(wrap((d >> low).wrapping_mul(inverse), rest))
}

/// `phi` as a [`Ring`] if it is the counter described above, else none.
fn matched(
    context: &Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &Function,
    phi: InstId,
) -> Option<Ring> {
    let one = function.instruction(phi);
    let width = context.types.int_bits(one.ty).filter(|&bits| (2..=64).contains(&bits))?;
    let &[a, Operand::Block(block_a), b, Operand::Block(block_b)] = &one.operands[..] else { return None };
    let (start_slot, start, next, latch) = match (constant(context, a), constant(context, b)) {
        (Some(start), None) => (0, start, def(function, b)?, block_b),
        (None, Some(start)) => (2, start, def(function, a)?, block_a),
        _ => return None,
    };
    let _ = block_a;
    let header = function.parent(phi)?;
    let counter = function.instruction(phi).result?;
    let add = function.instruction(next);
    if add.opcode != Opcode::Binary(BinaryOp::Add) || function.parent(next) != Some(latch) {
        return None;
    }
    let &[base, step] = &add.operands[..] else { return None };
    if base != Operand::Value(counter) {
        return None;
    }
    let step = wrap(constant(context, step)?, width);
    let after = add.result?;

    // The readers of the counter: the add, one `and`, maybe the exit test.
    let mut and = None;
    let mut icmp = None;
    for user in function.users(counter).iter() {
        let kind = &function.instruction(user.user).opcode;
        match kind {
            _ if user.user == next => {}
            Opcode::Binary(BinaryOp::And) if and.is_none() && user.index == 0 => and = Some(user.user),
            Opcode::ICmp(_) if icmp.is_none() => icmp = Some((user.user, 0u128)),
            _ => return None,
        }
    }
    for user in function.users(after).iter() {
        match function.instruction(user.user).opcode {
            Opcode::Phi if user.user == phi => {}
            Opcode::ICmp(_) if icmp.is_none() => icmp = Some((user.user, step)),
            _ => return None,
        }
    }
    let and = and?;
    let (icmp, offset) = icmp?;
    let test = function.instruction(icmp);
    let &[tested, limit] = &test.operands[..] else { return None };
    let exit = wrap(constant(context, limit)?, width);
    let mut readers = function.users(test.result?).iter();
    let (Some(branch), None) = (readers.next(), readers.next()) else { return None };
    // The test is the latch's, and its backedge is the one taken on a miss.
    let Opcode::ICmp(predicate) = test.opcode else { return None };
    let (hit, miss) = match function.instruction(branch.user).operands[..] {
        [_, Operand::Block(taken), Operand::Block(other)] => match predicate {
            IntPredicate::Eq => (taken, other),
            IntPredicate::Ne => (other, taken),
            _ => return None,
        },
        _ => return None,
    };
    if function.parent(icmp) != Some(latch)
        || function.parent(branch.user) != Some(latch)
        || miss != header
        || hit == header
        || tested != Operand::Value(if offset == 0 { counter } else { after })
    {
        return None;
    }

    // The `and`: a constant mask, read only as the index of `gep`s of one size.
    let masked = function.instruction(and);
    let mask = wrap(constant(context, masked.operands[1])?, width);
    let mut size = None;
    let mut geps = Vec::new();
    for user in function.users(masked.result?).iter() {
        let Opcode::GetElementPtr { source } = function.instruction(user.user).opcode else { return None };
        let bytes = layout.alloc_size(&context.types, source);
        if user.index != 1 || function.instruction(user.user).operands.len() != 2 || *size.get_or_insert(bytes) != bytes
        {
            return None;
        }
        geps.push(user.user);
    }
    let size = size.filter(|bytes| bytes.is_power_of_two() && *bytes >= 2)?;
    let shift = size.trailing_zeros();
    if shift >= width || mask >= (1u128 << (width - 1)) >> shift {
        return None;
    }

    // The scaled counter must hit the exit on the same trip.
    let gap = exit.wrapping_sub(offset).wrapping_sub(start);
    let trips = first_hit(step, gap, width)?;
    if first_hit(step, gap, width - shift) != Some(trips) {
        return None;
    }
    Some(Ring { phi, start_slot, next, and, geps, icmp, exit, mask, step, start, width, shift })
}

fn int(
    context: &Context,
    like: llrm_mir::types::TypeId,
    value: u128,
    width: u32,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty: like, kind: ConstantKind::Int(wrap(value, width)) }))
}

fn rewrite(
    context: &Context,
    function: &mut Function,
    ring: &Ring,
) {
    let ty = function.instruction(ring.phi).ty;
    let scaled = |value: u128| int(context, ty, value << ring.shift, ring.width);
    function.set_operand(ring.phi, ring.start_slot, scaled(ring.start));
    function.set_operand(ring.next, 1, scaled(ring.step));
    function.set_flags(ring.next, Flags::default());
    function.set_operand(ring.and, 1, scaled(ring.mask));
    function.set_operand(ring.icmp, 1, scaled(ring.exit));
    let bytes = context.types.int(8);
    for &gep in &ring.geps {
        function.set_opcode(gep, Opcode::GetElementPtr { source: bytes });
    }
}

impl FunctionPass for MaskScale {
    fn name(&self) -> &'static str {
        "maskscale"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if !outer.target().tti().is_some_and(|tti| tti.scaled_index_insns() > 0) {
            return PreservedAnalyses::all();
        }
        let rings: Vec<Ring> = {
            let function = &*unit.function;
            function
                .walk()
                .map(|(_, inst)| inst)
                .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
                .filter_map(|phi| matched(unit.context, unit.layout, function, phi))
                .collect()
        };
        for ring in &rings {
            rewrite(unit.context, unit.function, ring);
        }
        if rings.is_empty() {
            PreservedAnalyses::all()
        } else {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        }
    }
}

#[cfg(test)]
#[path = "maskscale_tests.rs"]
mod tests;
