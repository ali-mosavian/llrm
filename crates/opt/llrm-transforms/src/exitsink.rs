//! An add, subtract or extension in a loop that only exit phis read, moved to
//! those exits: LLVM's LICM sinking an instruction only used outside the loop.
//!
//! # How it works
//! A candidate is an `add`, `sub`, `sext`, `zext` or `trunc` in a loop whose
//! result is read only by phis outside the loop, one of which has this value as
//! its only input (an LCSSA exit phi). The operation is computed in the exit,
//! in place of the phi, and the loop's copy goes once nothing reads it. Each
//! other exit that reads it takes its own copy in its turn.
//!
//! An operand the loop defines is read through an exit phi, as LCSSA has it
//! (reusing one that exists); an invariant operand is read directly. The
//! rewrite repeats to a fixed point, so a chain of updates moves together.
//!
//! ```text
//!  loop:  %p1 = add %p, 2          loop:  (the add is gone)
//!         ...                             ...
//!  exit:  %r = phi [%p1, latch]    exit:  %q = phi [%p, latch]
//!                             ==>        %r = add %q, 2
//! ```
//!
//! The last trip's update of a pointer strength reduction carried is computed
//! once, on the way out, and so is the `sext` of a narrow counter whose value
//! `work += i` reads after the loop (mandel: `inc cx; movsx eax,cx; cmp cx,20h`
//! every trip).
//!
//! # Example
//! Before (`looped("add i16 %i, 40", "%i.next")`):
//!
//! ```mir
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %i.next, %b1 ]
//!   %i.next = add i16 %i, 1
//!   %last = add i16 %i, 40
//!   %more = icmp ult i16 %i.next, %n
//!   br i1 %more, label %b1, label %b2
//!
//! b2:
//!   %e = phi i16 [ %last, %b1 ]
//!   ret i16 %e
//! }
//! ```
//!
//! After, `b2` as the test asserts it (`%last` leaves `b1` once nothing reads
//! it):
//!
//! ```mir
//! b2:
//!   %i.lcssa = phi i16 [ %i, %b1 ]
//!   %0 = add i16 %i.lcssa, 40
//!   ret i16 %0
//! }
//! ```
//!
//! From unit test `test_final_update_moves_to_the_exit` in `exitsink_tests.rs`.
//!
//! # Runs
//! Step 4 of [`crate::indvars`]: on whenever `indvars` is, in the fixed point
//! and after the loop-header copy. Not at -O0; off with `-fno-strength-reduce`.
//!
//! # Keeps
//! Blocks and edges are unchanged; dominators and loops are preserved.
//!
//! # See also
//! [`crate::lcssa`]; LLVM LICM `sinkRegion`.

use llrm_analysis::cfg;
use llrm_analysis::cfg::Around;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};

use crate::edges;
use crate::lcssa::{arms, exit_phi, from_arms, place_phi};

/// Every add and subtract only an exit reads, moved there; whether any was.
/// `loops` are the body's: a sink moves an instruction between blocks and
/// leaves the blocks and edges as they are.
pub fn sunk(
    function: &mut Function,
    loops: &[Loop],
) -> bool {
    let mut changed = false;
    while let Some((loop_, phi, op)) = _sinkable(function, loops) {
        _sink(function, &loop_, phi, op);
        changed = true;
    }
    changed
}

/// An exit's phi of one input from `loop_`, and the add, subtract or
/// extension in the loop that input is, which only exit phis read.
fn _sinkable(
    function: &Function,
    loops: &[Loop],
) -> Option<(Loop, InstId, InstId)> {
    for loop_ in loops.iter().cloned() {
        // A candidate phi has an input from the loop: it is in a block the loop
        // exits to, found by the edges and not by every phi of the function.
        let exits = loop_.exits(function).into_iter().map(|(_, to)| to).collect::<std::collections::BTreeSet<_>>();
        for &block in function
            .layout()
            .iter()
            .filter(|&&block| exits.contains(&cfg::id(block)) && !loop_.body.contains(&cfg::id(block)))
        {
            for phi in edges::phis(function, block) {
                llrm_mir::counts::bump(llrm_mir::counts::Event::ExitSinkPhis);
                let [(Operand::Value(value), from)] = arms(function, phi)[..] else { continue };
                let ValueDef::Instruction(op) = function.value(value).def else { continue };
                let inside = |inst: InstId| function.parent(inst).is_some_and(|at| loop_.body.contains(&cfg::id(at)));
                if loop_.body.contains(&cfg::id(from))
                    && inside(op)
                    && matches!(
                        function.instruction(op).opcode,
                        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub)
                            | Opcode::Cast(CastOp::SExt | CastOp::ZExt | CastOp::Trunc)
                    )
                    && function.users(value).iter().all(|one| {
                        function.instruction(one.user).opcode == Opcode::Phi
                            && function.parent(one.user).is_some_and(|at| !loop_.body.contains(&cfg::id(at)))
                    })
                {
                    return Some((loop_, phi, op));
                }
            }
        }
    }
    None
}

/// `op` computed in place of `phi`, its operands the loop defines read
/// through the exit's phis.
fn _sink(
    function: &mut Function,
    loop_: &Loop,
    phi: InstId,
    op: InstId,
) {
    let exit = function.parent(phi).expect("a placed phi");
    let [(_, from)] = arms(function, phi)[..] else { unreachable!("one input") };
    let mut operands = Vec::new();
    for operand in function.instruction(op).operands.clone() {
        let defined = match operand {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Instruction(inst) => {
                    function.parent(inst).filter(|at| loop_.body.contains(&cfg::id(*at))).map(|_| value)
                }
                ValueDef::Argument(_) => None,
            },
            _ => None,
        };
        let Some(value) = defined else {
            operands.push(operand);
            continue;
        };
        let incoming = from_arms(&[(Operand::Value(value), from)]);
        let exported =
            match edges::phis(function, exit).into_iter().find(|&one| function.instruction(one).operands == incoming) {
                Some(existing) => existing,
                None => {
                    let export = exit_phi(function, value);
                    function.set_operands(export, incoming);
                    place_phi(function, exit, export).expect("a placed exit");
                    export
                }
            };
        operands.push(Operand::Value(function.instruction(exported).result.expect("a phi's value")));
    }
    let instruction = function.instruction(op);
    let moved =
        function.create_instruction(instruction.opcode.clone(), instruction.ty, operands, instruction.flags, None);
    let first =
        function.iter_block(exit).find(|&one| function.instruction(one).opcode != Opcode::Phi).expect("a terminator");
    function.insert(moved, Position::Before(first)).expect("a placed block");
    let result = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(function.instruction(moved).result.expect("a value")));
    function.set_operands(phi, Vec::new());
    function.erase(phi).expect("its uses were replaced");
    // Each other exit that reads it takes its own copy, in its turn.
    if function.users(function.instruction(op).result.expect("a value")).is_empty() {
        function.erase(op).expect("only exit phis read it");
    }
}

#[cfg(test)]
#[path = "exitsink_tests.rs"]
mod tests;
