//! A call a function makes to itself as the last thing it does becomes a
//! branch to its own entry with the arguments rewritten.
//!
//! # How it works
//! A site is a block that ends in `ret`, or in a `br` to a join block holding
//! only phis and a `ret` (the frontend's one return), where the last work
//! before the end is a self call: same callee, type and calling convention,
//! not marked `notail`. Only work with no effect may follow the call, and it
//! moves above it.
//!
//! The call's result may also feed one `add`, `mul`, `and`, `or` or `xor`
//! that is returned (`return f(n - 1) + x`). Then the loop carries an
//! accumulator: a phi in the loop header that starts at the operation's
//! identity (0, 1 for `mul`, all ones for `and`), takes the other operand of
//! the operation on each trip, and is combined with the value of every other
//! `ret`. Of two calls summed, the later is the tail and the earlier stays.
//! All sites of one function must use the same operation; sites of another
//! keep their calls.
//!
//! The entry block keeps the `alloca`s and branches to a new header
//! `tailrecurse`, which takes the rest of the entry. Each parameter that some
//! site changes gets a phi there (the entry's value, then each site's
//! argument); a parameter every site passes on unchanged gets none, and keeps
//! `noalias`, which the changed ones lose.
//!
//! ```text
//!   before                        after
//!   b0: ...                       b0:  (allocas) br tailrecurse
//!       %v = call f(%n-1)         tailrecurse:
//!       %s = add %v, %x             %n'  = phi [%n, b0], [%n-1, tailrecurse]
//!       ret %s                      %acc = phi [0, b0], [%acc+%x, ...]
//!                                   ...
//!                                   br tailrecurse     ; was the call
//!                                 other exits: ret (%acc + value)
//! ```
//!
//! The pass refuses a function that has a frame object whose address escapes
//! (`ExposedFrames`: a callee could read what the next trip overwrites), an
//! `alloca` outside the entry or of a dynamic size, a `byval`, `byref`,
//! `inalloca` or `sret` parameter or argument, or a call to a `returns_twice`
//! routine.
//!
//! Profitability: the carried values (every parameter the body reads, plus
//! the accumulator) must stay in registers. Where a call remains in the loop
//! they must fit in the target's registers kept across a call with one to
//! spare; otherwise in any register (`profit::registers`, [`crate::spill`]).
//! Spilled, the loop costs the stores and reloads the pushed arguments were.
//! A target that prices no registers always passes. `LLRM_DEBUG=tailrec`
//! prints the carried count and the room.
//!
//! # Example
//! Before:
//! ```mir
//! define ... @pipeline.body(i16 %0, i16 %1) ... {
//! b1:
//!   %2 = icmp eq i16 %0, 0
//!   br i1 %2, label %b3, label %b2
//!
//! b2:
//!   %3 = add nsw i16 %1, %0
//!   %4 = sub nsw i16 %0, 1
//!   %5 = call regparm3cc addrspace(1) i16 @"_sumto@3"(i16 %4, i16 %3)
//!   br label %b4
//!
//! b3:
//!   br label %b4
//!
//! b4:
//!   %6 = phi i16 [ %5, %b2 ], [ %1, %b3 ]
//!   ret i16 %6
//! }
//! ```
//! After:
//! ```mir
//! define ... @pipeline.body(i16 %0, i16 %1) ... {
//! b1:
//!   br label %tailrecurse
//!
//! tailrecurse:
//!   %2 = phi i16 [ %0, %b1 ], [ %6, %b2 ]
//!   %3 = phi i16 [ %1, %b1 ], [ %5, %b2 ]
//!   %4 = icmp eq i16 %2, 0
//!   br i1 %4, label %b3, label %b2
//!
//! b2:
//!   %5 = add nsw i16 %3, %2
//!   %6 = sub nsw i16 %2, 1
//!   br label %tailrecurse
//!
//! b3:
//!   br label %b4
//!
//! b4:
//!   %7 = phi i16 [ %3, %b3 ]
//!   ret i16 %7
//! }
//! ```
//! The self call is a branch to the new header `tailrecurse`; `%2` and `%3`
//! carry `n` and the accumulated `x` round the loop.
//!
//! From `--dump` of `sumto` (`if (n == 0) return x; return sumto(n - 1, x +
//! n);`) at -O2 -m16, stages `dead` and `tailrec`.
//!
//! # Runs
//! In the function pipeline after `gvn::EqualCalls`, so two equal calls are
//! one first, and before `loopsimplify`, so the loop it makes goes to the
//! loop passes. Once per round of the fixed point; at -O2 and above, off at
//! -O0 and -O1 and by `-fno-optimize-sibling-calls`.
//!
//! # Needs
//! `ExposedFrames` for the escape check; `profit::registers` for the room.
//!
//! # Keeps
//! Nothing: a change invalidates every analysis.
//!
//! # See also
//! LLVM's TailRecursionElimination and gcc's `tree-tailcall.cc`, which also
//! accumulate through associative operations.

use std::collections::BTreeSet;

use llrm_analysis::manager::ExposedFrames;
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::{Context, GlobalId};
use llrm_mir::edit::Position;
use llrm_mir::facts::Fact;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags, Opcode, Tail};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::counting;
use crate::edges;
use crate::lcssa::{arms, from_arms};
use crate::spill::Room;

pub struct TailRecursion;

impl FunctionPass for TailRecursion {
    fn name(&self) -> &'static str {
        "tailrec"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let Some(id) = unit.id else { return PreservedAnalyses::all() };
        if unit.function.is_declaration()
            || !unit.function.walk().any(|(_, inst)| memory::callee(unit.context, unit.function, inst) == Some(id))
        {
            return PreservedAnalyses::all();
        }
        let exposed = analyses.get::<ExposedFrames>(unit.context, unit.layout, unit.function);
        let outer = analyses.outer();
        let room = crate::profit::registers(outer);
        if eliminated(unit.context, outer.callees(), &outer.globals, unit.function, id, exposed.is_empty(), &room) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// A self call that ends its block's work.
struct Site {
    block: BlockId,
    call: InstId,
    /// Where the block goes on to return, if not by `ret` itself.
    join: Option<BlockId>,
    /// What combines the call's result with an operand on the way out.
    step: Option<Step>,
    /// Work after the call that does not use it: it moves above.
    after: Vec<InstId>,
}

struct Step {
    inst: InstId,
    op: BinaryOp,
}

/// `function` with each call to itself that is the last thing it does made a
/// branch to its entry; whether any was. `private`: no frame object's address
/// is exposed, so a callee cannot read what the frame holds and the next trip
/// may reuse it.
pub fn eliminated(
    context: &Context,
    callees: &Callees,
    globals: &[GlobalValue],
    function: &mut Function,
    id: GlobalId,
    private: bool,
    room: &Room,
) -> bool {
    if !private || !fixed_frame(function) || by_copy(function) || returns_twice(context, globals, function) {
        return false;
    }
    let mut found = sites(context, callees, function, id);
    // One accumulating operation per function: a site of another keeps its
    // call.
    let operation = found.iter().find_map(|site| site.step.as_ref().map(|step| step.op));
    found.retain(|site| site.step.as_ref().is_none_or(|step| Some(step.op) == operation));
    if found.is_empty() || !fits(function, &found, operation, room) {
        return false;
    }
    rewrite(context, function, &found, operation);
    true
}

/// Whether the loop's carried values stay in registers. Every parameter the
/// body reads, and the accumulator, live round the loop; where a call is left
/// in it they must outlast the call, in the registers the target keeps across
/// one (`across_call`) with one to spare, and else in any register.
/// Spilled, they cost the stores and reloads the call's argument pushes were:
/// bench's quicksort, hanoi and fib on a target of six registers and two kept
/// across a call ran up to 25% more memory operands in a loop than in the
/// recursion.
fn fits(
    function: &Function,
    found: &[Site],
    operation: Option<BinaryOp>,
    room: &Room,
) -> bool {
    if !room.priced() {
        return true;
    }
    let read = function.parameters().iter().filter(|&&parameter| !function.users(parameter).is_empty()).count() as i64;
    let carried = read + i64::from(operation.is_some());
    let kept = found.iter().map(|site| site.call).collect::<BTreeSet<_>>();
    let calls = function
        .walk()
        .any(|(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Call(_)) && !kept.contains(&inst));
    let limit = if calls { room.across_call } else { room.registers };
    llrm_support::debug!(
        "tailrec",
        "carried {carried}, room {limit} ({} registers, {} across a call)",
        room.registers,
        room.across_call
    );
    carried < limit || (!calls && carried <= limit)
}

/// Whether every `alloca` is in the entry block and of one size: a dynamic one
/// would grow with each trip.
fn fixed_frame(function: &Function) -> bool {
    let entry = function.entry();
    function
        .walk()
        .all(
            |(block, inst)| match function.instruction(inst).opcode {
                Opcode::Alloca { .. } => {
                    Some(block) == entry
                        && function.instruction(inst).operands.iter().all(|one| matches!(one, Operand::Constant(_)))
                }
                _ => true,
            },
        )
}

/// Whether a parameter or an argument is a copy the caller makes in memory
/// (`byval`, `sret`, ...): the next trip's would be a store to what the first
/// trip's callee wrote through.
fn by_copy(function: &Function) -> bool {
    let copying = |attrs: &[Attribute]| {
        attrs
            .iter()
            .any(
                |attr| matches!(
                    attr,
                    Attribute::Type(name, _) if matches!(name.as_str(), "byval" | "byref" | "inalloca" | "sret")
                ),
            )
    };
    function.parameter_attrs.iter().any(|attrs| copying(attrs))
        || function.walk().any(|(_, inst)| {
            matches!(
                &function.instruction(inst).opcode,
                Opcode::Call(info) if info.argument_attrs.iter().any(|attrs| copying(attrs))
            )
        })
}

/// Whether `function` calls a routine that returns twice (`setjmp`): one frame
/// for every trip is not the frame a second return finds.
fn returns_twice(
    context: &Context,
    globals: &[GlobalValue],
    function: &Function,
) -> bool {
    function
        .walk()
        .any(
            |(_, inst)| {
                let Opcode::Call(info) = &function.instruction(inst).opcode else { return false };
                memory::has(&info.attrs, "returns_twice")
                    || memory::callee(context, function, inst)
                        .and_then(|one| globals.get(one.0 as usize)?.function())
                        .is_some_and(|one| memory::has(&one.attrs, "returns_twice"))
            },
        )
}

/// The self calls of `function` that end a block.
fn sites(
    context: &Context,
    callees: &Callees,
    function: &Function,
    id: GlobalId,
) -> Vec<Site> {
    function.layout().iter().filter_map(|&block| site(context, callees, function, id, block)).collect()
}

/// `block`'s self call, if its last work is one.
fn site(
    context: &Context,
    callees: &Callees,
    function: &Function,
    id: GlobalId,
    block: BlockId,
) -> Option<Site> {
    let end = function.terminator(block)?;
    let (value, join) = match function.instruction(end).opcode {
        Opcode::Ret => (function.instruction(end).operands.first().copied(), None),
        Opcode::Br if function.instruction(end).operands.len() == 1 => {
            let Operand::Block(to) = function.instruction(end).operands[0] else { return None };
            (returned_from(function, to, block)?, Some(to))
        }
        _ => return None,
    };
    let work: Vec<InstId> = function.block(block).iter().filter(|&one| one != end).collect();
    let is_call = |inst: InstId| {
        let op = function.instruction(inst);
        let Opcode::Call(info) = &op.opcode else { return false };
        memory::callee(context, function, inst) == Some(id)
            && info.function_type == function.ty
            && info.calling_convention == function.calling_convention
            && info.tail != Tail::NoTail
            && op.operands.len() == function.parameters().len() + 1
    };
    let defined = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) if function.parent(inst) == Some(block) => Some(inst),
            _ => None,
        },
        _ => None,
    };
    let (call, step) = match value {
        None => (
            work.iter()
                .rposition(|&inst| !memory::speculatable(context, callees, function, inst))
                .map(|at| work[at])
                .filter(|&inst| is_call(inst))?,
            None,
        ),
        Some(returned) => {
            let made = defined(returned)?;
            if is_call(made) {
                (made, None)
            } else {
                let Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)) =
                    function.instruction(made).opcode
                else {
                    return None;
                };
                let [left, right] = function.instruction(made).operands[..] else { return None };
                // Of two calls summed (fib), the later is the tail; the
                // earlier's result is what it adds to.
                let at = |inst: InstId| work.iter().position(|&one| one == inst);
                let call = match (defined(left).filter(|&one| is_call(one)), defined(right).filter(|&one| is_call(one)))
                {
                    (Some(call), None) | (None, Some(call)) => call,
                    (Some(first), Some(second)) => {
                        if at(first) > at(second) {
                            first
                        } else {
                            second
                        }
                    }
                    _ => return None,
                };
                (call, Some(Step { inst: made, op }))
            }
        }
    };
    // The call's result goes nowhere but on to the way out.
    if let Some(result) = function.instruction(call).result
        && function.users(result).len() != 1
    {
        return None;
    }
    if let Some(step) = &step
        && function.users(function.instruction(step.inst).result?).len() != 1
    {
        return None;
    }
    let at = work.iter().position(|&inst| inst == call)?;
    let after: Vec<InstId> =
        work[at + 1..].iter().copied().filter(|&inst| Some(inst) != step.as_ref().map(|step| step.inst)).collect();
    // Only work with no effect may stand between the call and the way out, and
    // it moves above the call.
    if !after.iter().all(|&inst| memory::speculatable(context, callees, function, inst))
        || step.as_ref().is_some_and(|step| !work[at + 1..].contains(&step.inst))
    {
        return None;
    }
    Some(Site { block, call, join, step, after })
}

/// What a join block returns to `from`, which branches to it, if the block is
/// only phis and a `ret`: `Some(None)` for `ret void`.
fn returned_from(
    function: &Function,
    join: BlockId,
    from: BlockId,
) -> Option<Option<Operand>> {
    let end = function.terminator(join).filter(|&one| function.instruction(one).opcode == Opcode::Ret)?;
    if join == from
        || function
            .block(join)
            .instructions()
            .iter()
            .any(|&one| one != end && function.instruction(one).opcode != Opcode::Phi)
    {
        return None;
    }
    let value = function.instruction(end).operands.first().copied();
    let Some(Operand::Value(returned)) = value else { return Some(value) };
    match function.value(returned).def {
        ValueDef::Instruction(phi) if function.parent(phi) == Some(join) => {
            Some(arms(function, phi).into_iter().find(|&(_, block)| block == from).map(|(value, _)| value))
        }
        _ => Some(value),
    }
}

/// The accumulator's value before the first trip: what `op` leaves a number as.
fn identity(
    context: &Context,
    op: BinaryOp,
    bits: u32,
) -> Operand {
    let one = if op == BinaryOp::Mul {
        1
    } else if op == BinaryOp::And {
        -1
    } else {
        0
    };
    counting::constant(context, &BigInt::from(one), bits)
}

fn rewrite(
    context: &Context,
    function: &mut Function,
    found: &[Site],
    operation: Option<BinaryOp>,
) {
    let void = context.types.void();
    let entry = function.entry().expect("a body");
    // The entry keeps the frame; the rest is the loop's header.
    let header = function.create_block(Some("tailrecurse"));
    function.insert_block(header, Some(entry)).expect("a new block");
    let first =
        function.block(entry).iter().find(|&inst| !matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }));
    let rest: Vec<_> = function
        .block(entry)
        .iter()
        .filter(|&inst| !matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }))
        .collect();
    function.move_run(&rest, header).expect("a placed instruction");
    function.replace_block_uses_with(entry, header);
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(header)], Flags::default(), None);
    function.insert(jump, Position::End(entry)).expect("a placed block");
    let first = first.expect("the call is in the body");

    // One phi per parameter the calls change, and one for the accumulator.
    let mut phis = Vec::new();
    let unchanged: Vec<bool> = function
        .parameters()
        .iter()
        .enumerate()
        .map(|(at, &parameter)| {
            found.iter().all(|site| function.instruction(site.call).operands[at] == Operand::Value(parameter))
        })
        .collect();
    for (at, parameter) in function.parameters().to_vec().into_iter().enumerate() {
        if unchanged[at] {
            phis.push(None);
            continue;
        }
        let ty = function.value(parameter).ty;
        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
        function.insert(phi, Position::Before(first)).expect("a placed instruction");
        let value = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        function.replace_all_uses_with(parameter, value);
        function.set_operands(phi, vec![Operand::Value(parameter), Operand::Block(entry)]);
        phis.push(Some(phi));
    }
    let accumulator = operation.map(|op| {
        let ty =
            function.instruction(found.iter().find(|site| site.step.is_some()).expect("an accumulating site").call).ty;
        let bits = context.types.int_bits(ty).expect("an integer accumulator");
        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
        function.insert(phi, Position::Before(first)).expect("a placed instruction");
        let start = identity(context, op, bits);
        function.set_operands(phi, vec![start, Operand::Block(entry)]);
        (phi, op, ty)
    });
    let accumulated =
        accumulator.map(|(phi, ..)| Operand::Value(function.instruction(phi).result.expect("a phi's value")));

    let mut joins = Vec::new();
    for site in found {
        // A body of one block moved to the header with the rest of the entry.
        let block = if site.block == entry { header } else { site.block };
        for &inst in &site.after {
            function.move_to(inst, Position::Before(site.call)).expect("a placed instruction");
        }
        let arguments = function.instruction(site.call).operands.clone();
        let carried = match (&site.step, accumulator, accumulated) {
            (Some(step), Some((_, op, ty)), Some(acc)) => {
                // Read now: a parameter it names has become its phi.
                let result = function.instruction(site.call).result.map(Operand::Value);
                let with = function
                    .instruction(step.inst)
                    .operands
                    .iter()
                    .copied()
                    .find(|&one| Some(one) != result)
                    .expect("a step has another operand");
                let made = function.create_instruction(Opcode::Binary(op), ty, vec![acc, with], Flags::default(), None);
                function.insert(made, Position::Before(site.call)).expect("a placed instruction");
                function.instruction(made).result.map(Operand::Value)
            }
            _ => accumulated,
        };
        let end = function.terminator(block).expect("a terminator");
        if let Some(join) = site.join {
            for phi in edges::phis(function, join) {
                let kept: Vec<_> = arms(function, phi).into_iter().filter(|&(_, from)| from != block).collect();
                function.set_operands(phi, from_arms(&kept));
            }
            joins.push(join);
        }
        function.erase(end).expect("a terminator has no result");
        if let Some(step) = &site.step {
            function.erase(step.inst).expect("its user is gone");
        }
        function.erase(site.call).expect("its user is gone");
        for (&phi, &argument) in phis.iter().zip(&arguments) {
            let Some(phi) = phi else { continue };
            let mut operands = function.instruction(phi).operands.clone();
            operands.extend([argument, Operand::Block(block)]);
            function.set_operands(phi, operands);
        }
        if let (Some((phi, ..)), Some(carried)) = (accumulator, carried) {
            let mut operands = function.instruction(phi).operands.clone();
            operands.extend([carried, Operand::Block(block)]);
            function.set_operands(phi, operands);
        }
        let back = function.create_instruction(Opcode::Br, void, vec![Operand::Block(header)], Flags::default(), None);
        function.insert(back, Position::End(block)).expect("a placed block");
    }

    // A join nothing reaches any more goes.
    for join in joins {
        if function.layout().contains(&join) && function.predecessors(join).is_empty() {
            for inst in function.block(join).instructions().iter().rev().copied().collect::<Vec<_>>() {
                function.erase(inst).expect("only the block's own instructions use them");
            }
            function.erase_block(join).expect("an empty block nothing names");
        }
    }
    // What returns now returns what the trips before it accumulated.
    if let Some((phi, op, ty)) = accumulator {
        let acc = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        for block in function.layout().to_vec() {
            let Some(end) = function.terminator(block).filter(|&one| function.instruction(one).opcode == Opcode::Ret)
            else {
                continue;
            };
            let value = function.instruction(end).operands[0];
            let sum = function.create_instruction(Opcode::Binary(op), ty, vec![acc, value], Flags::default(), None);
            function.insert(sum, Position::Before(end)).expect("a placed terminator");
            let sum = Operand::Value(function.instruction(sum).result.expect("a sum"));
            function.set_operand(end, 0, sum);
        }
    }
    // The next trip's argument is another pointer where the calls change it,
    // which `noalias` does not cover; where they pass it on as it came, it
    // is the same pointer, and its promise (Nib's `&mut`) stands.
    for (attrs, same) in function.parameter_attrs.iter_mut().zip(&unchanged) {
        if *same {
            continue;
        }
        attrs.retain(|attr| Fact::of_attribute(attr) != Some(Fact::NoAlias));
    }
}

#[cfg(test)]
#[path = "tailrec_tests.rs"]
mod tests;
