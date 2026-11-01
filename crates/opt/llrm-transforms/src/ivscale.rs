//! A counter read only as `i << k` counts in bytes (DEVIATE LSR4).
//!
//! Real mode has no scale-4 address without the address-size prefix, so LSR
//! keeps `shl i, k` for the addresses and the counter beside it: the shift runs
//! every trip and holds a register. `4 * i` steps by 4 as well as `i` does, so
//! it can be the counter, and the bound is shifted once, ahead of the loop.
//! LSR's own formulas keep the scaled use (`scale 4` is legal for the
//! description and the pin chooses it); the cost is in the code the scaled
//! form becomes, which LSR prices once per scaled register.
//!
//! # How it works
//! An innermost-or-not loop in simplify form whose counter `i` is a phi
//! `{start,+,1}` of one width `w`, read only by its latch add, shifts `shl i,
//! k` (one `k`) and one `icmp eq/ne` of `i` or `i + 1` against a loop-invariant
//! bound `B`, which the latch branches on. The counter becomes `i << k`: start
//! and step shifted, each shift replaced by the counter, `B << k` made in the
//! preheader.
//!
//! The rewrite is exact when `i << k` hits the bound on the trip `i` does: `B`
//! is at least `start` (plus one for the test of `i + 1`) and below `2^(w-k)`,
//! so the counter reaches `B` before either wraps. Both come from `B`'s
//! unsigned range under the loop's guards (SCEV), so a bound the analysis
//! cannot limit is left alone. The latch add loses its wrap flags. Only a
//! target that prices a scaled index (`Tti::scaled_index_insns`) runs it.
//! Cost: O(uses) per loop.
//!
//! DEVIATE LSR4 (the pin's LSR chooses the scaled form and its code prices it
//! once, not per trip; exit: the scaled form is priced and emitted alike, then
//! this is deleted; owner llrm-w12-lsr).
//!
//! # Example
//! The butterfly loop of c's `stft_single` at m16 (schematic); `half` is
//! `m >> 1` with `m` at most 256:
//!
//! ```text
//!  %j = phi i16 [ 0 ], [ %next ]            %j = phi i16 [ 0 ], [ %next ]
//!  %b = shl i16 %j, 2                       %next = add i16 %j, 4
//!  ...load [%a + %b]                  ==>   ...load [%a + %j]
//!  %next = add i16 %j, 1                    %c = icmp eq i16 %next, %half4
//!  %c = icmp eq i16 %next, %half            (%half4 = shl %half, 2 in the preheader)
//! ```
//!
//! # Runs
//! After `lsr`, `differences` and `maskscale`, in the strength slot
//! ([`crate::pipeline::strength_slot`]); option `ivscale`.

use llrm_analysis::scev::analysis::ScevSession;
use llrm_mir::constantrange::ConstantRange;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};

use crate::ivutil::{constant, def, int};

/// `size`: the function is optimized for size, where the counter's wider step
/// (`add 4` for `inc`) costs what the shift saves.
pub struct IvScale {
    pub size: bool,
}

/// What was matched: the counter's pieces and the constants it rewrites.
struct Scaled {
    phi: InstId,
    start_slot: usize,
    start: u128,
    /// 1 where the test reads `i + 1`.
    offset: u128,
    next: InstId,
    shifts: Vec<InstId>,
    icmp: InstId,
    bound_slot: usize,
    bound: Operand,
    preheader: BlockId,
    shift: u32,
    width: u32,
}

/// `ScevSession` over the headers of `function`'s loops, innermost first.
pub fn scaled_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let headers: Vec<BlockId> = {
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        crate::loopfilter::retain(&mut headers);
        headers
    };
    // The trip counts first: a range asked while one is worked out misses it.
    for &header in &headers {
        let cx = env.cx(&*context, &*function);
        se.get_small_constant_max_trip_count(&cx, header);
    }
    se.forget_cached_ranges();
    let mut changed = false;
    for header in headers {
        let phis: Vec<InstId> =
            function.iter_block(header).filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect();
        for phi in phis {
            let matched_here = {
                let cx = env.cx(&*context, &*function);
                let Some(l) = cx.loops.get_loop_for(header) else { continue };
                if !l.is_loop_simplify_form(cx.function) {
                    continue;
                }
                matched(context, function, &l, header, phi).map(|one| {
                    let s = se.get_scev(&cx, one.bound);
                    let guarded = se.apply_loop_guards(&cx, s, header);
                    (se.get_unsigned_range(&cx, guarded), one)
                })
            };
            let found = matched_here.and_then(|(range, one)| {
                llrm_support::debug!("ivscale", "counter shifted by {}: bound range {range:?}", one.shift);
                exact(&range, &one).then_some(one)
            });
            if let Some(one) = found {
                rewrite(context, function, &one);
                changed = true;
            }
        }
    }
    changed
}

/// The scaled counter hits the bound on the trip the counter does: the bound
/// is not below the counter's first value it can be tested at, and the scaled
/// bound does not wrap.
fn exact(
    range: &ConstantRange,
    one: &Scaled,
) -> bool {
    if range.is_full_set() || range.is_empty_set() || one.shift >= one.width {
        return false;
    }
    range.get_unsigned_min() >= one.start + one.offset && range.get_unsigned_max() < (1u128 << (one.width - one.shift))
}

/// `phi` as a [`Scaled`] counter if it is the one described above.
fn matched(
    context: &Context,
    function: &Function,
    l: &llrm_mir::loopinfo::Loop<'_>,
    header: BlockId,
    phi: InstId,
) -> Option<Scaled> {
    let one = function.instruction(phi);
    let width = context.types.int_bits(one.ty).filter(|&bits| (2..=64).contains(&bits))?;
    let &[a, Operand::Block(block_a), b, Operand::Block(block_b)] = &one.operands[..] else { return None };
    let preheader = l.get_loop_preheader(function)?;
    let latch = l.get_loop_latch(function)?;
    let (start_slot, start, next) = match (constant(context, a), constant(context, b)) {
        (Some(start), None) if block_a == preheader && block_b == latch => (0, start, def(function, b)?),
        (None, Some(start)) if block_b == preheader && block_a == latch => (2, start, def(function, a)?),
        _ => return None,
    };
    let counter = one.result?;
    let add = function.instruction(next);
    if add.opcode != Opcode::Binary(BinaryOp::Add) || function.parent(next) != Some(latch) {
        return None;
    }
    let &[base, step] = &add.operands[..] else { return None };
    if base != Operand::Value(counter) || constant(context, step) != Some(1) {
        return None;
    }
    let after = add.result?;

    // The readers of the counter: the add, shifts by one constant, one test.
    let mut shifts = Vec::new();
    let mut shift = None;
    let mut icmp = None;
    for user in function.users(counter).iter() {
        let reader = function.instruction(user.user);
        match reader.opcode {
            _ if user.user == next => {}
            Opcode::Binary(BinaryOp::Shl) if user.index == 0 => {
                let by = constant(context, reader.operands[1])? as u32;
                if *shift.get_or_insert(by) != by {
                    return None;
                }
                shifts.push(user.user);
            }
            Opcode::ICmp(_) if icmp.is_none() => icmp = Some((user.user, counter)),
            _ => return None,
        }
    }
    for user in function.users(after).iter() {
        match function.instruction(user.user).opcode {
            Opcode::Phi if user.user == phi => {}
            Opcode::ICmp(_) if icmp.is_none() => icmp = Some((user.user, after)),
            _ => return None,
        }
    }
    let (icmp, tested) = icmp?;
    let shift = shift.filter(|&by| by >= 1)?;
    let test = function.instruction(icmp);
    let &[x, y] = &test.operands[..] else { return None };
    let (bound_slot, bound) = if x == Operand::Value(tested) { (1, y) } else { (0, x) };
    if !l.is_loop_invariant(function, bound) {
        return None;
    }
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
    {
        return None;
    }
    let offset = u128::from(tested == after);
    Some(Scaled { phi, start_slot, start, offset, next, shifts, icmp, bound_slot, bound, preheader, shift, width })
}

fn rewrite(
    context: &Context,
    function: &mut Function,
    one: &Scaled,
) {
    let ty = function.instruction(one.phi).ty;
    let scaled = |value: u128| int(context, ty, value << one.shift, one.width);
    function.set_operand(one.phi, one.start_slot, scaled(one.start));
    function.set_operand(one.next, 1, scaled(1));
    function.set_flags(one.next, Flags::default());
    let bound = match constant(context, one.bound) {
        Some(value) => scaled(value),
        None => {
            let terminator = function.terminator(one.preheader).expect("a preheader terminator");
            let made = function.create_instruction(
                Opcode::Binary(BinaryOp::Shl),
                ty,
                vec![one.bound, int(context, ty, u128::from(one.shift), one.width)],
                Flags::NUW,
                None,
            );
            function.insert(made, Position::Before(terminator)).expect("a placed terminator");
            Operand::Value(function.instruction(made).result.expect("a shift value"))
        }
    };
    function.set_operand(one.icmp, one.bound_slot, bound);
    let counter = function.instruction(one.phi).result.expect("a counter value");
    for &shift in &one.shifts {
        let result = function.instruction(shift).result.expect("a shift value");
        function.replace_all_uses_with(result, Operand::Value(counter));
        function.erase(shift).expect("a shift nothing reads");
    }
}

impl FunctionPass for IvScale {
    fn name(&self) -> &'static str {
        "ivscale"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if self.size {
            return PreservedAnalyses::all();
        }
        let outer = std::rc::Rc::clone(analyses.outer());
        if !outer.target().tti().is_some_and(|tti| tti.scaled_index_insns() > 0) {
            return PreservedAnalyses::all();
        }
        let mut session = ScevSession::within(unit.context, &outer.globals, &outer.metadata, unit.function);
        if scaled_in(&mut session, unit.context, unit.layout, unit.function, &outer) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "ivscale_tests.rs"]
mod tests;
