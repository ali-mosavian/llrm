//! Pointers that step through memory a byte at a time, as a base and a byte
//! index, before instruction selection.
//!
//! # How it works
//! A loop with a pointer `p` that starts at `s` and steps by `c` bytes is
//! `p = s + c * trips`. On a target whose indexed load adds an index of `n`
//! bits to a base (`Machine::index_register_bits`), the sum is cheaper kept in
//! two parts: an index `i` that steps by `c`, and a base that moves by `2^n`
//! where the index wraps. The loop's pointers share the one index.
//!
//! The pass rewrites the loops of one latch and one way in whose pointers are
//! used only as the address of a byte load or store (or of a constant offset
//! from it). Pointers it cannot rewrite keep their own steps. A loop rotated to
//! load the next byte before it tests steps in the latch at that load, and the
//! load reads the next page's base through a join block after it.
//!
//! DEVIATE (6502-6): it is not LLVM's. `MOSIndexIV` (llvm-mos) rewrites a
//! pointer to a base and an 8-bit index only where `ScalarEvolution` proves the
//! index stays under 256; this one handles the wrap with a block run once every
//! `256 / c` trips. Reason: the string and memory routines of the C library
//! walk a pointer of unknown extent. Exit: `MOSIndexIV` translated for the case
//! `ScalarEvolution` proves under `2^n`; the wrap block stays as the deviation.
//!
//! # Example
//! `strcpy`'s loop, in IR:
//!
//! ```text
//! loop: %d1 = phi [%d, pre], [%d2, loop]    %s1 = phi [%s, pre], [%s2, loop]
//!       %c = load i8, ptr %s1 ; store i8 %c, ptr %d1
//!       %s2 = gep %s1, 1 ; %d2 = gep %d1, 1 ; br %c != 0, loop, exit
//! ```
//!
//! becomes
//!
//! ```text
//! loop: %bd = phi [%d, pre], [%bd, step], [%bd2, bump]    (same for %bs)
//!       %i = phi i8 [0, pre], [%i1, step], [%i1, bump]
//!       %ps = gep %bs, zext %i ; %pd = gep %bd, zext %i
//!       %c = load i8, ptr %ps ; store i8 %c, ptr %pd ; br %c != 0, step, exit
//! step: %i1 = add i8 %i, 1 ; br %i1 == 0, bump, loop
//! bump: %bd2 = gep %bd, 256 ; %bs2 = gep %bs, 256 ; br loop
//! ```
//!
//! # Runs
//! In a target's driver, after the mid-end, for a target whose
//! `Machine::index_register_bits` is set: the width of the index its indexed
//! addressing adds to a base (8 for `lda (base),y`).

use llrm_analysis::cfg::{self, loops_of};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::TypeId;

/// A pointer of the loop that steps by a constant: its phi in the header, its
/// start, the step instruction, the step in bytes.
struct Walker {
    phi: InstId,
    value: ValueId,
    start: Operand,
    next: InstId,
    step: u128,
    /// The type of its step's constant: the width of an offset from a pointer.
    offset: TypeId,
    /// The instructions that read it as an address.
    accesses: Vec<(InstId, usize)>,
    /// The loads and stores that read its next value, in the latch.
    ahead: Vec<(InstId, usize)>,
    /// The instructions outside the loop that read it.
    after: Vec<(InstId, usize)>,
    /// The one-armed phis outside the loop that read it.
    exits: Vec<InstId>,
}

/// Every loop of `module`'s functions that can walk by page; the names of the
/// functions that have one.
pub fn indexed(
    module: &mut Module,
    index_bits: u32,
) -> Result<Vec<String>, String> {
    let context = &mut module.context;
    let mut changed = Vec::new();
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let mut done = false;
        let loops = loops_of(function);
        // innermost loops only: their blocks are not another loop's header
        let headers: Vec<i64> = loops.iter().map(|one| one.header).collect();
        for one in loops
            .iter()
            .filter(|one| one.body.iter().filter(|at| **at != one.header && headers.contains(at)).count() == 0)
        {
            done |=
                rewrite(context, function, one.header, one.latches.iter().copied().collect(), &one.body, index_bits)?;
        }
        if let (true, Some(name)) = (done, global.name.as_deref()) {
            changed.push(name.to_owned());
        }
    }
    Ok(changed)
}

fn int(
    context: &mut Context,
    ty: TypeId,
    value: u128,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }))
}

fn constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    }
}

fn arms(
    function: &Function,
    phi: InstId,
) -> Vec<(Operand, BlockId)> {
    function
        .instruction(phi)
        .operands
        .chunks(2)
        .map(|pair| match pair {
            [value, Operand::Block(from)] => (*value, *from),
            _ => unreachable!("a phi's arms are value and block"),
        })
        .collect()
}

fn set_arms(
    function: &mut Function,
    phi: InstId,
    arms: &[(Operand, BlockId)],
) {
    function.set_operands(phi, arms.iter().flat_map(|(value, from)| [*value, Operand::Block(*from)]).collect());
}

/// The pointers of the loop (`header`, one `latch`) that step by a constant and
/// are read only as addresses.
fn walkers(
    context: &Context,
    byte: TypeId,
    max_step: u128,
    function: &Function,
    header: BlockId,
    preheader: BlockId,
    latch: BlockId,
    in_loop: &dyn Fn(BlockId) -> bool,
) -> Vec<Walker> {
    let byte = Some(byte);
    let mut found = Vec::new();
    for phi in crate::edges::phis(function, header) {
        let Some(value) = function.instruction(phi).result else { continue };
        let [(first, from_first), (second, from_second)] = arms(function, phi)[..] else { continue };
        let (start, next_value) = if from_first == preheader && from_second == latch {
            (first, second)
        } else if from_second == preheader && from_first == latch {
            (second, first)
        } else {
            continue;
        };
        let Operand::Value(next_value) = next_value else { continue };
        let ValueDef::Instruction(next) = function.value(next_value).def else { continue };
        let (step, offset) = match (&function.instruction(next).opcode, &function.instruction(next).operands[..]) {
            (Opcode::GetElementPtr { source }, [base, index])
                if Some(*source) == byte && *base == Operand::Value(value) =>
            {
                match (constant(context, *index), function.operand_type(context, *index)) {
                    (Some(step), Some(offset)) if (1..=max_step).contains(&step) => (step, offset),
                    _ => continue,
                }
            }
            _ => continue,
        };
        if function.parent(next) != Some(latch) {
            continue;
        }
        // the next pointer is read by this phi and by loads and stores after
        // it in the latch (a loop rotated to read the next element ahead)
        let mut ahead = Vec::new();
        let mut kept = true;
        for one in function.users(next_value) {
            let user = function.instruction(one.user);
            match (&user.opcode, one.index) {
                _ if one.user == phi => {}
                (Opcode::Load { .. }, 0) if Some(user.ty) == byte && function.parent(one.user) == Some(latch) => {
                    ahead.push((one.user, 0))
                }
                (Opcode::Store { .. }, 1)
                    if function.operand_type(context, user.operands[0]) == byte
                        && function.parent(one.user) == Some(latch) =>
                {
                    ahead.push((one.user, 1))
                }
                _ => kept = false,
            }
        }
        if !kept {
            continue;
        }
        let mut accesses = Vec::new();
        let mut after = Vec::new();
        let mut exits = Vec::new();
        let mut plain = true;
        for one in function.users(value) {
            let user = function.instruction(one.user);
            let at = function.parent(one.user);
            match (&user.opcode, one.index) {
                _ if one.user == next => {}
                (Opcode::Load { .. }, 0) if Some(user.ty) == byte => accesses.push((one.user, 0)),
                (Opcode::Store { .. }, 1) if function.operand_type(context, user.operands[0]) == byte => {
                    accesses.push((one.user, 1))
                }
                (Opcode::Phi, _) if at.is_some_and(|at| !in_loop(at)) && user.operands.len() == 2 => {
                    exits.push(one.user)
                }
                (opcode, _) if *opcode != Opcode::Phi && at.is_some_and(|at| !in_loop(at)) => {
                    after.push((one.user, one.index as usize))
                }
                _ => plain = false,
            }
        }
        if plain && !(accesses.is_empty() && ahead.is_empty()) {
            found.push(Walker { phi, value, start, next, step, offset, accesses, ahead, after, exits });
        }
    }
    found
}

fn rewrite(
    context: &mut Context,
    function: &mut Function,
    header_at: i64,
    latches: Vec<i64>,
    body: &llrm_mir::forest::Body,
    index_bits: u32,
) -> Result<bool, String> {
    let [latch_at] = latches[..] else { return Ok(false) };
    let (header, latch) = (cfg::block(header_at), cfg::block(latch_at));
    let in_loop = |at: BlockId| body.contains(&cfg::id(at));
    let outside: Vec<BlockId> = function.predecessors(header).into_iter().filter(|from| !in_loop(*from)).collect();
    let [preheader] = outside[..] else { return Ok(false) };
    let Some(jump) = function.terminator(latch) else { return Ok(false) };
    if function.instruction(jump).opcode != Opcode::Br
        || function.successors(latch).iter().filter(|to| **to == header).count() != 1
    {
        return Ok(false);
    }
    let byte = context.types.int(8);
    let mut found = walkers(context, byte, (1 << (index_bits - 1)) - 1, function, header, preheader, latch, &in_loop);
    // one step for all: the first walker's
    let Some((step, word)) = found.first().map(|one| (one.step, one.offset)) else { return Ok(false) };
    found.retain(|one| one.step == step);

    let others: Vec<InstId> = crate::edges::phis(function, header)
        .into_iter()
        .filter(|phi| !found.iter().any(|one| one.phi == *phi))
        .collect();
    let index_ty = context.types.int(index_bits);
    let flag = context.types.int(1);
    let void = function.instruction(jump).ty;
    let new = |function: &mut Function, block: Position, opcode: Opcode, ty, operands: Vec<Operand>, flags| {
        let made = function.create_instruction(opcode, ty, operands, flags, None);
        function.insert(made, block).map_err(|why| why.to_string())?;
        Ok::<_, String>(made)
    };
    let value_of =
        |function: &Function, made: InstId| Operand::Value(function.instruction(made).result.expect("a result"));

    // A loop that reads a pointer's next value ahead steps in the latch at the
    // first such read and joins in a block after it. Any other steps on the
    // back edge: in the latch if it jumps to the header, else in a block of
    // its own.
    let order: Vec<InstId> = function.iter_block(latch).collect();
    let split = found
        .iter()
        .filter(|one| !one.ahead.is_empty())
        .filter_map(|one| order.iter().position(|&inst| inst == one.next))
        .min()
        .map(|at| order[at]);
    let (stepper, join) = match split {
        Some(at) => {
            let tail = crate::edges::split_block_at(function, latch, at);
            function.set_block_name(tail, "page-tail");
            let joined = function.terminator(latch).ok_or("a terminated latch")?;
            function.erase(joined).map_err(|why| why.to_string())?;
            (latch, tail)
        }
        None if function.instruction(jump).operands.len() == 1 => {
            function.erase(jump).map_err(|why| why.to_string())?;
            (latch, header)
        }
        None => {
            let step_block = function.create_block(Some("page-step"));
            function.insert_block(step_block, Some(latch)).map_err(|why| why.to_string())?;
            let at = function
                .instruction(jump)
                .operands
                .iter()
                .position(|one| *one == Operand::Block(header))
                .ok_or("a back edge")?;
            function.set_operand(jump, at, Operand::Block(step_block));
            (step_block, header)
        }
    };
    let bump = function.create_block(Some("page-bump"));
    function.insert_block(bump, Some(stepper)).map_err(|why| why.to_string())?;

    // after the split: it takes the header's first instruction with it when
    // the header is the latch and the step is its first, and leaves only phis
    let first = function
        .iter_block(header)
        .find(|&one| function.instruction(one).opcode != Opcode::Phi)
        .map_or(Position::End(header), Position::Before);

    // the index and the bases, phis at the top of the header and of the join
    let zero = int(context, index_ty, 0);
    let step_value = int(context, index_ty, step);
    let index = new(function, first, Opcode::Phi, index_ty, Vec::new(), Flags::default())?;
    let mut bases = Vec::new();
    for walker in &found {
        let ty = function.operand_type(context, Operand::Value(walker.value)).ok_or("a typed pointer")?;
        bases.push(new(function, first, Opcode::Phi, ty, Vec::new(), Flags::default())?);
    }
    let gep = Opcode::GetElementPtr { source: byte };
    let mut onward = Vec::new();
    if join != header {
        let top = function.iter_block(join).next().ok_or("a terminated join")?;
        for walker in &found {
            let ty = function.operand_type(context, Operand::Value(walker.value)).ok_or("a typed pointer")?;
            onward.push(new(function, Position::Before(top), Opcode::Phi, ty, Vec::new(), Flags::default())?);
        }
    }

    // the stepper steps the index; where it wraps the bases move on a page
    let next_index = new(
        function,
        Position::End(stepper),
        Opcode::Binary(BinaryOp::Add),
        index_ty,
        vec![value_of(function, index), step_value],
        Flags::default(),
    )?;
    let wrapped = if step == 1 {
        new(
            function,
            Position::End(stepper),
            Opcode::ICmp(IntPredicate::Eq),
            flag,
            vec![value_of(function, next_index), zero],
            Flags::default(),
        )?
    } else {
        new(
            function,
            Position::End(stepper),
            Opcode::ICmp(IntPredicate::Ult),
            flag,
            vec![value_of(function, next_index), step_value],
            Flags::default(),
        )?
    };
    new(
        function,
        Position::End(stepper),
        Opcode::Br,
        void,
        vec![value_of(function, wrapped), Operand::Block(bump), Operand::Block(join)],
        Flags::default(),
    )?;
    let page = int(context, word, 1 << index_bits);
    let mut moved = Vec::new();
    for (walker, base) in found.iter().zip(&bases) {
        let ty = function.operand_type(context, Operand::Value(walker.value)).ok_or("a typed pointer")?;
        moved.push(new(
            function,
            Position::End(bump),
            gep.clone(),
            ty,
            vec![value_of(function, *base), page],
            Flags::default(),
        )?);
    }
    new(function, Position::End(bump), Opcode::Br, void, vec![Operand::Block(join)], Flags::default())?;

    // the other phis of the header now take the back edge from the step and
    // the bump
    if join == header {
        for &phi in &others {
            let arms: Vec<_> = arms(function, phi)
                .into_iter()
                .flat_map(
                    |(value, from)| {
                        if from == latch { vec![(value, stepper), (value, bump)] } else { vec![(value, from)] }
                    },
                )
                .collect();
            set_arms(function, phi, &arms);
        }
    }
    // the phis: the index starts at 0, a base at the pointer's start
    let next = value_of(function, next_index);
    if join == header {
        set_arms(function, index, &[(zero, preheader), (next, stepper), (next, bump)]);
        for ((walker, base), moved) in found.iter().zip(&bases).zip(&moved) {
            let own = value_of(function, *base);
            let page_on = value_of(function, *moved);
            set_arms(function, *base, &[(walker.start, preheader), (own, stepper), (page_on, bump)]);
        }
    } else {
        set_arms(function, index, &[(zero, preheader), (next, join)]);
        for (((walker, base), moved), onward) in found.iter().zip(&bases).zip(&moved).zip(&onward) {
            let (own, ahead) = (value_of(function, *base), value_of(function, *onward));
            set_arms(function, *base, &[(walker.start, preheader), (ahead, join)]);
            set_arms(function, *onward, &[(own, stepper), (value_of(function, *moved), bump)]);
        }
    }
    // the sum is made where the pointer was read, for the selector to fold
    // into the access
    for (n, (walker, base)) in found.iter().zip(&bases).enumerate() {
        let ty = function.operand_type(context, Operand::Value(walker.value)).ok_or("a typed pointer")?;
        let sum = |function: &mut Function, position: Position, base: InstId, by: InstId| -> Result<Operand, String> {
            let wide = new(
                function,
                position,
                Opcode::Cast(CastOp::ZExt),
                word,
                vec![value_of(function, by)],
                Flags::default(),
            )?;
            let made = new(
                function,
                position,
                gep.clone(),
                ty,
                vec![value_of(function, base), value_of(function, wide)],
                Flags::default(),
            )?;
            Ok(value_of(function, made))
        };
        for &(user, at_operand) in walker.accesses.iter().chain(&walker.after) {
            let made = sum(function, Position::Before(user), *base, index)?;
            function.set_operand(user, at_operand, made);
        }
        for &(user, at_operand) in &walker.ahead {
            let made = sum(function, Position::Before(user), onward[n], next_index)?;
            function.set_operand(user, at_operand, made);
        }
        for &phi in &walker.exits {
            let block = function.parent(phi).ok_or("a placed phi")?;
            let top = function
                .iter_block(block)
                .find(|&one| function.instruction(one).opcode != Opcode::Phi)
                .ok_or("a terminated block")?;
            let made = sum(function, Position::Before(top), *base, index)?;
            let result = function.instruction(phi).result.ok_or("a phi result")?;
            function.replace_all_uses_with(result, made);
            function.erase(phi).map_err(|why| why.to_string())?;
        }
        // the step reads the pointer and the pointer reads the step: the
        // phi lets go of it first
        function.set_operands(walker.phi, Vec::new());
        function.erase(walker.next).map_err(|why| why.to_string())?;
        function.erase(walker.phi).map_err(|why| why.to_string())?;
    }
    Ok(true)
}
