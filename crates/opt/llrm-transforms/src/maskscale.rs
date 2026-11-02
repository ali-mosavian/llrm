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
//! A narrow counter that `lsr` made of the mask (`zext i6 p to i16`, `shl 7`)
//! beside the wide trip counter that tests the exit is merged the same way:
//! one counter `p' = 2^7 * p` of the wide type, read through `and (2^6-1) * 2^7`,
//! tests the exit at the scaled trip count. The trip counter is dropped.
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

/// A narrow counter read as `zext; shl` beside a wide trip counter: the pieces
/// to merge them into one scaled counter.
struct Merge {
    phi: InstId,
    start_slot: usize,
    next: InstId,
    zext: InstId,
    shl: InstId,
    trip_phi: InstId,
    trip_next: InstId,
    icmp: InstId,
    tested_after: bool,
    wide: llrm_mir::types::TypeId,
    width: u32,
    narrow: u32,
    shift: u32,
    start: u128,
    step: u128,
    exit: u128,
}

/// The only user of `value`, if it has one.
fn sole_user(
    function: &Function,
    value: llrm_mir::module::ValueId,
) -> Option<llrm_mir::module::Use> {
    let mut users = function.users(value).iter();
    let (one, None) = (users.next()?, users.next()) else { return None };
    Some(*one)
}

/// `phi` as a [`Merge`] if it is the narrow counter described above, else none.
fn merged(
    context: &Context,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &Function,
    phi: InstId,
) -> Option<Merge> {
    let one = function.instruction(phi);
    let narrow = context.types.int_bits(one.ty).filter(|&bits| (2..=32).contains(&bits))?;
    let &[a, Operand::Block(block_a), b, Operand::Block(block_b)] = &one.operands[..] else { return None };
    let (start_slot, start, next, latch) = match (constant(context, a), constant(context, b)) {
        (Some(start), None) => (0, start, def(function, b)?, block_b),
        (None, Some(start)) => (2, start, def(function, a)?, block_a),
        _ => return None,
    };
    let header = function.parent(phi)?;
    let counter = one.result?;
    let add = function.instruction(next);
    if add.opcode != Opcode::Binary(BinaryOp::Add) || function.parent(next) != Some(latch) {
        return None;
    }
    let &[base, step] = &add.operands[..] else { return None };
    if base != Operand::Value(counter) || sole_user(function, add.result?).map(|one| one.user) != Some(phi) {
        return None;
    }
    let step = wrap(constant(context, step)?, narrow);

    // Read by its add and one `zext` to the wide type; that read only by one
    // `shl` by a constant, read only as the index of byte `gep`s.
    let mut zext = None;
    for user in function.users(counter).iter() {
        match function.instruction(user.user).opcode {
            _ if user.user == next => {}
            Opcode::Cast(llrm_mir::opcode::CastOp::ZExt) if zext.is_none() => zext = Some(user.user),
            _ => return None,
        }
    }
    let zext = zext?;
    let wide = function.instruction(zext).ty;
    let width = context.types.int_bits(wide).filter(|&bits| bits > narrow && bits <= 64)?;
    let shl = sole_user(function, function.instruction(zext).result?)?;
    let shl_op = function.instruction(shl.user);
    let (Opcode::Binary(BinaryOp::Shl), 0) = (&shl_op.opcode, shl.index) else { return None };
    let shift = u32::try_from(constant(context, shl_op.operands[1])?).ok()?;
    let mask = wrap(u128::MAX, narrow);
    if shift == 0 || narrow + shift > width || (mask << shift) >= 1u128 << (width - 1) {
        return None;
    }
    for user in function.users(shl_op.result?).iter() {
        let Opcode::GetElementPtr { source } = function.instruction(user.user).opcode else { return None };
        if user.index != 1
            || function.instruction(user.user).operands.len() != 2
            || layout.alloc_size(&context.types, source) != 1
        {
            return None;
        }
    }

    // The wide trip counter of the loop: constant start and step, read by its
    // add and the exit test only.
    let header_phis: Vec<InstId> = function
        .iter_block(header)
        .filter(|&inst| inst != phi && function.instruction(inst).opcode == Opcode::Phi)
        .collect();
    header_phis.into_iter().find_map(|trip_phi| {
        let q = function.instruction(trip_phi);
        if q.ty != wide {
            return None;
        }
        let &[c, Operand::Block(block_c), d, Operand::Block(block_d)] = &q.operands[..] else { return None };
        let (trip_start, trip_next) = match (constant(context, c), constant(context, d)) {
            (Some(start), None) if block_d == latch && block_c != latch => (start, def(function, d)?),
            (None, Some(start)) if block_c == latch && block_d != latch => (start, def(function, c)?),
            _ => return None,
        };
        let qadd = function.instruction(trip_next);
        let &[qbase, qstep] = &qadd.operands[..] else { return None };
        if qadd.opcode != Opcode::Binary(BinaryOp::Add)
            || function.parent(trip_next) != Some(latch)
            || qbase != Operand::Value(q.result?)
        {
            return None;
        }
        let trip_step = wrap(constant(context, qstep)?, width);
        let trip_after = qadd.result?;
        let mut icmp = None;
        for user in function.users(q.result?).iter() {
            match function.instruction(user.user).opcode {
                _ if user.user == trip_next => {}
                Opcode::ICmp(_) if icmp.is_none() => icmp = Some((user.user, false)),
                _ => return None,
            }
        }
        for user in function.users(trip_after).iter() {
            match function.instruction(user.user).opcode {
                Opcode::Phi if user.user == trip_phi => {}
                Opcode::ICmp(_) if icmp.is_none() => icmp = Some((user.user, true)),
                _ => return None,
            }
        }
        let (icmp, tested_after) = icmp?;
        let test = function.instruction(icmp);
        let &[tested, limit] = &test.operands[..] else { return None };
        let limit = wrap(constant(context, limit)?, width);
        let Opcode::ICmp(predicate) = test.opcode else { return None };
        let mut readers = function.users(test.result?).iter();
        let (Some(branch), None) = (readers.next(), readers.next()) else { return None };
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
            || tested != Operand::Value(if tested_after { trip_after } else { q.result? })
        {
            return None;
        }
        // The trip the wide counter exits on, and the scaled counter's first
        // hit of the scaled test value: the same trip.
        let offset = if tested_after { trip_step } else { 0 };
        let trips = first_hit(trip_step, limit.wrapping_sub(offset).wrapping_sub(trip_start), width)?;
        let scaled_step = wrap(step << shift, width);
        let scaled_start = wrap(start << shift, width);
        let scaled_offset = if tested_after { scaled_step } else { 0 };
        let exit = wrap(scaled_start.wrapping_add(scaled_step.wrapping_mul(trips)).wrapping_add(scaled_offset), width);
        let gap = exit.wrapping_sub(scaled_offset).wrapping_sub(scaled_start);
        if first_hit(scaled_step, gap, width) != Some(trips) {
            return None;
        }
        Some(Merge {
            phi,
            start_slot,
            next,
            zext,
            shl: shl.user,
            trip_phi,
            trip_next,
            icmp,
            tested_after,
            wide,
            width,
            narrow,
            shift,
            start,
            step,
            exit,
        })
    })
}

fn merge(
    context: &Context,
    function: &mut Function,
    found: &Merge,
) {
    let scaled = |value: u128| int(context, found.wide, value << found.shift, found.width);
    function.set_type(found.phi, found.wide);
    function.set_type(found.next, found.wide);
    function.set_operand(found.phi, found.start_slot, scaled(found.start));
    function.set_operand(found.next, 1, scaled(found.step));
    function.set_flags(found.next, Flags::default());
    // The read: `zext; shl` is the masked, scaled counter.
    let mask = wrap(u128::MAX, found.narrow) << found.shift;
    function.set_opcode(found.zext, Opcode::Binary(BinaryOp::And));
    function.set_operands(
        found.zext,
        vec![Operand::Value(function.instruction(found.phi).result.expect("a phi")), int(context, found.wide, mask, found.width)],
    );
    let read = function.instruction(found.zext).result.expect("an and");
    let shifted = function.instruction(found.shl).result.expect("a shl");
    function.replace_all_uses_with(shifted, Operand::Value(read));
    // The exit test moves to the scaled counter; the trip counter goes.
    let counter = function.instruction(found.phi).result.expect("a phi");
    let after = function.instruction(found.next).result.expect("an add");
    // The add may follow the test it now feeds.
    function.move_to(found.next, llrm_mir::edit::Position::Before(found.icmp)).expect("the add moves in its block");
    function.set_operand(found.icmp, 0, Operand::Value(if found.tested_after { after } else { counter }));
    function.set_operand(found.icmp, 1, int(context, found.wide, found.exit, found.width));
    function.erase(found.shl).expect("the shl is unused");
    function.erase_all(&[found.trip_phi, found.trip_next]).expect("the trip counter is unused");
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
        let merges: Vec<Merge> = {
            let function = &*unit.function;
            function
                .walk()
                .map(|(_, inst)| inst)
                .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
                .filter_map(|phi| merged(unit.context, unit.layout, function, phi))
                .collect()
        };
        for found in &merges {
            merge(unit.context, unit.function, found);
        }
        if rings.is_empty() && merges.is_empty() {
            PreservedAnalyses::all()
        } else {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        }
    }
}

#[cfg(test)]
#[path = "maskscale_tests.rs"]
mod tests;
