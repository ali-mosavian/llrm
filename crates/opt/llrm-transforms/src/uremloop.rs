//! CodeGenPrepare's `foldURemOfLoopIncrement`: the remainder of a loop's
//! counter by a loop-invariant amount is a second counter that wraps
//! ([CodeGenPrepare.cpp#L2227](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2227)).
//!
//! # How it works
//! `for (i = Start; i < End; ++i) r = (i + K) % M` with `M` and `K` invariant
//! and no constant `M` (a constant remainder is a multiply). The counter must
//! be a header phi of two inputs that steps by one with a `nuw` add. When the
//! first remainder, `(Start + K) % M`, simplifies away (`Start` is 0, say), the
//! division in the loop is replaced by a phi that counts up and drops to zero
//! on reaching `M`:
//!
//! ```text
//!  loop: %i = phi [0, %pre], [%n, %loop]      loop: %i = phi [0, %pre], [%n, %loop]
//!        %r = urem %i, %m               ==>         %r = phi [0, %pre], [%s, %loop]
//!        ...                                        %a = add nuw %r, 1
//!        %n = add nuw %i, 1                         %c = icmp eq %a, %m
//!                                                   %s = select %c, 0, %a
//! ```
//!
//! MIRROR: `isRemOfLoopIncrementWithLoopInvariant`, `getIVIncrement` and
//! `foldURemOfLoopIncrement`.
//!
//! DEVIATE UREM1: `matchIncrement`'s `uadd.with.overflow` and `usub` forms are
//! not matched: nothing before this makes them (`combineToUAddWithOverflow` is
//! not translated). Exit: that translation.
//!
//! # Example
//! `test_a_remainder_of_a_counter_by_an_invariant_is_a_wrapping_counter` in
//! `uremloop_tests.rs` is the sketch above.
//!
//! # Runs
//! In `cgpaddr`'s run, before the store split.

use llrm_analysis::cfg::{self, Around};
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};

/// `L->isLoopInvariant(V)`: a constant, an argument, or a value made outside.
fn invariant(
    function: &Function,
    body: &impl Fn(BlockId) -> bool,
    operand: Operand,
) -> bool {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| !body(block)),
            _ => true,
        },
        _ => true,
    }
}

/// The first instruction of `block`'s defining `operand`, if it is one.
fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(def) => Some(def),
        _ => None,
    }
}

fn is_one(
    context: &Context,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Constant(id) if context.get(id).kind == ConstantKind::Int(1)
    )
}

/// `foldURemOfLoopIncrement(rem)`: whether it was replaced.
fn folded(
    context: &Context,
    layout: &DataLayout,
    loops: &[Loop],
    function: &mut Function,
    rem: InstId,
) -> bool {
    let one = function.instruction(rem).clone();
    if one.opcode != Opcode::Binary(BinaryOp::URem) {
        return false;
    }
    let &[incr, rem_amount] = &one.operands[..] else { return false };
    let Some(incr_def) = instruction_of(function, incr) else { return false };
    // `m_c_NUWAdd(m_Phi(PN), m_Value(AddOffset))`, else the phi itself.
    let (phi, add, offset) = if function.instruction(incr_def).opcode == Opcode::Phi {
        (incr_def, None, None)
    } else {
        let add = function.instruction(incr_def);
        if add.opcode != Opcode::Binary(BinaryOp::Add) || !add.flags.contains(Flags::NUW) {
            return false;
        }
        let &[x, y] = &add.operands[..] else { return false };
        let as_phi = |operand: Operand| {
            instruction_of(function, operand).filter(|&def| function.instruction(def).opcode == Opcode::Phi)
        };
        match (as_phi(x), as_phi(y)) {
            (Some(phi), _) => (phi, Some(incr_def), Some(y)),
            (None, Some(phi)) => (phi, Some(incr_def), Some(x)),
            _ => return false,
        }
    };
    if function.instruction(phi).operands.len() != 4 {
        return false;
    }
    // Only a header, and a trivially analyzable loop.
    let Some(header) = function.parent(phi) else { return false };
    let Some(the_loop) = loops.iter().find(|one| one.header == cfg::id(header)) else { return false };
    let (Some(preheader), 1) = (the_loop.preheader(function), the_loop.latches.len()) else { return false };
    let latch = *the_loop.latches.iter().next().expect("one latch");
    let in_body = |block: BlockId| the_loop.body.contains(&cfg::id(block));
    let Some(rem_block) = function.parent(rem) else { return false };
    if !in_body(rem_block) || !invariant(function, &in_body, rem_amount) {
        return false;
    }
    if offset.is_some_and(|offset| !invariant(function, &in_body, offset)) {
        return false;
    }
    // `getIVIncrement`: the latch's value is `add PN, 1` in the loop.
    let arms = llrm_analysis::occurrence::arms(function, phi);
    let from = |block: BlockId| arms.iter().find(|&&(_, at)| at == block).map(|&(value, _)| value);
    let (Some(start), Some(stepped)) = (from(cfg::block(preheader)), from(cfg::block(latch))) else { return false };
    let Some(inc) = instruction_of(function, stepped) else { return false };
    let Some(inc_block) = function.parent(inc) else { return false };
    let innermost = loops.iter().find(|one| one.body.contains(&cfg::id(inc_block)));
    if innermost.map(|one| one.header) != Some(the_loop.header) {
        return false;
    }
    let step = function.instruction(inc);
    let phi_value = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
    // `matchIncrement` of one, and `m_c_NUWAdd(PN, ...)` (DEVIATE UREM1).
    let &[left, step_by] = &step.operands[..] else { return false };
    if step.opcode != Opcode::Binary(BinaryOp::Add)
        || !step.flags.contains(Flags::NUW)
        || left != phi_value
        || !is_one(context, step_by)
    {
        return false;
    }
    // Only a non-constant remainder amount.
    if matches!(rem_amount, Operand::Constant(_)) {
        return false;
    }
    // The first remainder must simplify away.
    let mut first = start;
    if let Some(add) = add {
        let flags = function.instruction(add).flags;
        let add_flags = if flags.contains(Flags::NSW) { Flags::NUW.union(Flags::NSW) } else { Flags::NUW };
        let Some(sum) = llrm_mir::simplify::simplify_binop_operands(
            context,
            layout,
            function,
            BinaryOp::Add,
            add_flags,
            first,
            offset.expect("an add has an offset"),
        ) else {
            return false;
        };
        first = sum;
    }
    let Some(first) = llrm_mir::simplify::simplify_binop_operands(
        context,
        layout,
        function,
        BinaryOp::URem,
        Flags::default(),
        first,
        rem_amount,
    ) else {
        return false;
    };

    // The new counter, beside the old one; its step in the latch.
    let ty = one.ty;
    let i1 = context.types.int(1);
    let int = |value: u128| Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }));
    let new_rem = function.create_instruction(
        Opcode::Phi,
        ty,
        vec![first, Operand::Block(cfg::block(preheader)), first, Operand::Block(cfg::block(latch))],
        Flags::default(),
        None,
    );
    function.insert(new_rem, Position::Before(phi)).expect("a position before the phi");
    let new_value = Operand::Value(function.instruction(new_rem).result.expect("a phi's value"));
    let bump =
        function.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![new_value, int(1)], Flags::NUW, None);
    function.insert(bump, Position::Before(inc)).expect("a position before the increment");
    let bumped = Operand::Value(function.instruction(bump).result.expect("an add has a value"));
    let wrapped = function.create_instruction(
        Opcode::ICmp(IntPredicate::Eq),
        i1,
        vec![bumped, rem_amount],
        Flags::default(),
        None,
    );
    function.insert(wrapped, Position::Before(inc)).expect("a position before the increment");
    let wrapped_value = Operand::Value(function.instruction(wrapped).result.expect("a compare has a value"));
    let select =
        function.create_instruction(Opcode::Select, ty, vec![wrapped_value, int(0), bumped], Flags::default(), None);
    function.insert(select, Position::Before(inc)).expect("a position before the increment");
    let selected = Operand::Value(function.instruction(select).result.expect("a select has a value"));
    function.set_operand(new_rem, 2, selected);

    let old = function.instruction(rem).result.expect("a remainder has a value");
    function.replace_all_uses_with(old, new_value);
    function.erase(rem).expect("its uses were replaced");
    if let Some(add) = add
        && function.instruction(add).result.is_some_and(|result| function.users(result).is_empty())
    {
        function.erase(add).expect("an add nothing reads goes");
    }
    true
}

/// `optimizeURem` for each `urem` of `function`; whether any was replaced.
/// The loops are derived only for a function that has a `urem` (a pass over
/// every function paid for them: +0.24 M Ir on d_faces -O2).
pub fn remainders_folded(
    context: &Context,
    layout: &DataLayout,
    loops: impl FnOnce(&Function) -> Vec<Loop>,
    function: &mut Function,
) -> bool {
    let rems: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::URem))
        .collect();
    if rems.is_empty() {
        return false;
    }
    let loops = loops(function);
    if loops.is_empty() {
        return false;
    }
    let mut changed = false;
    for rem in rems {
        if !function.is_erased(rem) {
            changed |= folded(context, layout, &loops, function, rem);
        }
    }
    changed
}

#[cfg(test)]
#[path = "uremloop_tests.rs"]
mod tests;
