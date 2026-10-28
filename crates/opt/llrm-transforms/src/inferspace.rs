//! Replaces a far pointer made from a near one by the near one where its users
//! allow it, so an access needs no segment register loaded and held for it.
//!
//! # How it works
//! `addrspacecast` of a near pointer to a far space addresses what the near one
//! does, so a load, a store or a `gep` through it can use the near pointer and
//! drop the selector: no `les`, no segment register held across a loop.
//!
//! A far pointer that is a constant cast of a global (what forwarding a stored
//! far pointer leaves where its load was) is read the same way: a load or store
//! through it, or through steps of it that only memory reads, uses the global.
//!
//! A far value narrows when every user is one that does not need the selector:
//! a non-volatile load's or store's pointer, the base of a `gep`, a phi or
//! select of narrowing values, or a cast back to the near space (which is the
//! near pointer itself). Anything else (a call argument, a compare, a stored
//! pointer) needs the selector, and a phi or select that another far value or
//! such a user also reads is left alone: a far copy kept beside the near one
//! would cost a register.
//!
//! The near space a pointer narrows to follows where it points: the data space
//! for a global, the target's stack space for a stack object, whose far pointer
//! is SS:offset and which a data-space pointer read through DS would not
//! reach unless SS were DS. No such assumption is made. Phis and selects
//! narrow only where every value they join narrows to one space; a join of two
//! spaces stays far.
//!
//! DEVIATE LP-5 (no pin pass on x86: infer-address-spaces is a target pass
//! (none for x86); the near pointer under a far cast matters at m16 only; exit:
//! deleted if the m16 rows do not lose without it, else folded into LP-4; owner
//! w-6; review 2026-11-13).
//!
//! # Example
//! Before:
//!
//! ```mir
//! @g = global [64 x i16] zeroinitializer
//!
//! define i16 @f(i16 %i) {
//! b0:
//!   %w = addrspacecast ptr @g to ptr addrspace(1)
//!   %a = getelementptr i8, ptr addrspace(1) %w, i16 %i
//!   %v = load i16, ptr addrspace(1) %a
//!   ret i16 %v
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i16 @f(i16 %i) {
//! b0:
//!   %w = addrspacecast ptr @g to ptr addrspace(1)
//!   %0 = getelementptr i8, ptr @g, i16 %i
//!   %a = getelementptr i8, ptr addrspace(1) %w, i16 %i
//!   %v = load i16, ptr %0
//!   ret i16 %v
//! }
//! ```
//!
//! The load reads through a near `gep` of `@g`. The far `%w` and `%a` are left
//! for `dead`.
//!
//! From unit test `a_load_through_a_step_of_a_cast_global_is_a_near_load` in
//! `inferspace_tests.rs`, whose module also declares a 16-bit data layout with
//! address space 1 far.
//!
//! # Runs
//! In every body's pipeline before `gvn`, so a far pointer cast from a near one
//! is read as the near one; part of the fixed point. `narrowspace` uses
//! [`on_stack`].
//!
//! # Keeps
//! Dominators and loops stay valid.
//!
//! # See also
//! LLVM's InferAddressSpaces (InferAddressSpaces.cpp).

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::{ConstantExpr, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::spaces::Spaces;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::underlying;

pub struct InferAddressSpaces;

impl FunctionPass for InferAddressSpaces {
    fn name(&self) -> &'static str {
        "inferspace"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        // Blocks and edges are as they were.
        if inferred(unit.context, unit.layout, unit.function, analyses.outer().target().spaces()) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Whether `operand` is a stack object or steps from one: its far pointer
/// addresses SS, not DS.
pub fn on_stack(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    operand: Operand,
) -> bool {
    let (Operand::Value(root), _) = underlying(context, layout, function, operand) else { return false };
    matches!(
        function.value(root).def,
        ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Alloca { .. })
    )
}

/// The far space `ty` is a pointer into, when narrowing may drop its selector.
fn far(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
) -> Option<u32> {
    match context.types.get(ty) {
        Type::Pointer(space) if layout.is_pair(*space) && !layout.carries(*space) => Some(*space),
        _ => None,
    }
}

/// The near space `ty` is a pointer into, if it is one.
fn near(
    context: &Context,
    spaces: Spaces,
    ty: TypeId,
) -> Option<u32> {
    match context.types.get(ty) {
        Type::Pointer(space) if *space == spaces.data || *space == spaces.stack => Some(*space),
        _ => None,
    }
}

/// Where `v`'s value goes: how each of its users reads it.
#[derive(PartialEq)]
enum Reads {
    /// The pointer of a load or store, or the base of a step: it can read the
    /// near one.
    Narrows,
    /// A cast back to the near space.
    Drops,
    /// Anything that needs the selector.
    Needs,
}

/// What is known of the near space a far value narrows to.
#[derive(Clone, Copy, PartialEq)]
enum Space {
    /// Not yet: only itself, round a cycle, reaches it.
    Unknown,
    Near(u32),
    /// Two spaces meet in it, or a value that is neither.
    Neither,
}

fn met(
    one: Space,
    other: Space,
) -> Space {
    match (one, other) {
        (Space::Unknown, any) | (any, Space::Unknown) => any,
        (Space::Near(a), Space::Near(b)) if a == b => Space::Near(a),
        _ => Space::Neither,
    }
}

/// The near pointer a constant far cast is made of: `addrspacecast (ptr @g to
/// ptr addrspace(1))` is `@g`, a global's address in the data space, and a
/// constant step of one is the same step of the global (a field of a forwarded
/// struct).
fn near_of_constant(
    context: &Context,
    layout: &DataLayout,
    spaces: Spaces,
    operand: Operand,
) -> Option<Operand> {
    let Operand::Constant(id) = operand else { return None };
    near_constant(context, layout, spaces, id).map(Operand::Constant)
}

fn near_constant(
    context: &Context,
    layout: &DataLayout,
    spaces: Spaces,
    id: llrm_mir::context::ConstantId,
) -> Option<llrm_mir::context::ConstantId> {
    let constant = context.get(id);
    far(context, layout, constant.ty)?;
    match constant.kind.clone() {
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::AddrSpaceCast, value }) => {
            (near(context, spaces, context.get(value).ty) == Some(spaces.data)).then_some(value)
        }
        ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, nusw, nuw, operands }) => {
            let base = near_constant(context, layout, spaces, operands[0])?;
            let mut stepped = operands;
            stepped[0] = base;
            Some(context.constant(llrm_mir::context::Constant {
                ty: context.types.ptr(spaces.data),
                kind: ConstantKind::Expr(ConstantExpr::GetElementPtr {
                    source,
                    inbounds,
                    nusw,
                    nuw,
                    operands: stepped,
                }),
            }))
        }
        _ => None,
    }
}

/// Whether every user of `value` reads it through memory or steps it on to one
/// that does.
fn only_read_through(
    function: &Function,
    value: ValueId,
    depth: u32,
) -> bool {
    depth < 8
        && !function.users(value).is_empty()
        && function.users(value).iter().all(|one| match &function.instruction(one.user).opcode {
            Opcode::Load { volatile: false, .. } => one.index == 0,
            Opcode::Store { volatile: false, .. } => one.index == 1,
            Opcode::GetElementPtr { .. } if one.index == 0 => function
                .instruction(one.user)
                .result
                .is_some_and(|result| only_read_through(function, result, depth + 1)),
            _ => false,
        })
}

/// A load, a store or a step through a constant far cast of a global (what
/// forwarding a stored pointer leaves in place of the load) reads the global's
/// near address. `inferred` follows casts that are instructions only.
fn narrow_constant_casts(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    spaces: Spaces,
) -> bool {
    let walk: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let data = context.types.ptr(spaces.data);
    let mut near_steps: IdMap<ValueId, Operand> = IdMap::default();
    let mut changed = false;
    for inst in walk {
        let instruction = function.instruction(inst).clone();
        let base_of = |operand: Operand| match operand {
            Operand::Value(value) => near_steps.get(&value).copied(),
            constant => near_of_constant(context, layout, spaces, constant),
        };
        match instruction.opcode {
            Opcode::Load { volatile: false, .. } => {
                if let Some(base) = base_of(instruction.operands[0]) {
                    function.set_operand(inst, 0, base);
                    changed = true;
                }
            }
            Opcode::Store { volatile: false, .. } => {
                if let Some(base) = base_of(instruction.operands[1]) {
                    function.set_operand(inst, 1, base);
                    changed = true;
                }
            }
            Opcode::GetElementPtr { .. } => {
                let (Some(base), Some(result)) = (base_of(instruction.operands[0]), instruction.result) else {
                    continue;
                };
                if far(context, layout, instruction.ty).is_none() || !only_read_through(function, result, 0) {
                    continue;
                }
                let mut operands = instruction.operands.clone();
                operands[0] = base;
                let made =
                    function.create_instruction(instruction.opcode.clone(), data, operands, instruction.flags, None);
                function.insert(made, Position::Before(inst)).expect("placed");
                near_steps.insert(result, Operand::Value(function.instruction(made).result.expect("a step's value")));
            }
            _ => {}
        }
    }
    changed
}

fn inferred(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    spaces: Spaces,
) -> bool {
    let narrowed = narrow_constant_casts(context, layout, function, spaces);
    inferred_casts(context, layout, function, spaces) || narrowed
}

fn inferred_casts(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    spaces: Spaces,
) -> bool {
    let walk: Vec<(llrm_mir::module::BlockId, InstId)> = function.walk().collect();
    // The far pointers that are a near one cast: value -> the near pointer, the
    // space it narrows to.
    let mut seeds: BTreeMap<ValueId, (Operand, u32)> = BTreeMap::new();
    for &(_, inst) in &walk {
        let instruction = function.instruction(inst);
        let (Opcode::Cast(CastOp::AddrSpaceCast), Some(result)) = (&instruction.opcode, instruction.result) else {
            continue;
        };
        let source = instruction.operands[0];
        let Some(from) = function.operand_type(context, source).and_then(|ty| near(context, spaces, ty)) else {
            continue;
        };
        if far(context, layout, instruction.ty).is_none() {
            continue;
        }
        // Where its object is: the stack's selector is SS, DGROUP's is DS.
        let space = if from == spaces.stack || on_stack(context, layout, function, source) {
            spaces.stack
        } else {
            spaces.data
        };
        seeds.insert(result, (source, space));
    }
    if seeds.is_empty() {
        return false;
    }
    let near_types: BTreeMap<u32, TypeId> =
        [spaces.data, spaces.stack].into_iter().map(|space| (space, context.types.ptr(space))).collect();
    let context: &Context = context;
    // Far steps, phis and selects that may follow: by value.
    let mut chain: BTreeMap<ValueId, InstId> = BTreeMap::new();
    for &(_, inst) in &walk {
        let instruction = function.instruction(inst);
        if matches!(
            instruction.opcode,
            Opcode::GetElementPtr { .. } | Opcode::Phi | Opcode::Select
        ) {
            if let Some(result) = instruction.result {
                if far(context, layout, instruction.ty).is_some() {
                    chain.insert(result, inst);
                }
            }
        }
    }
    let pointers = |function: &Function, inst: InstId| -> Vec<Operand> {
        let instruction = function.instruction(inst);
        match instruction.opcode {
            Opcode::GetElementPtr { .. } => vec![instruction.operands[0]],
            Opcode::Select => vec![instruction.operands[1], instruction.operands[2]],
            _ => instruction.operands.chunks(2).map(|pair| pair[0]).collect(),
        }
    };
    let usage = |function: &Function, value: ValueId, user: InstId, index: u32| -> Reads {
        let instruction = function.instruction(user);
        match &instruction.opcode {
            Opcode::Load { volatile: false, .. } if index == 0 => Reads::Narrows,
            Opcode::Store { volatile: false, .. } if index == 1 => Reads::Narrows,
            Opcode::GetElementPtr { .. } if index == 0 => Reads::Narrows,
            Opcode::Phi | Opcode::Select if instruction.result.is_some_and(|one| chain.contains_key(&one)) => {
                Reads::Narrows
            }
            Opcode::Cast(CastOp::AddrSpaceCast)
                if near(context, spaces, instruction.ty).is_some() && function.value(value).ty != instruction.ty =>
            {
                Reads::Drops
            }
            _ => Reads::Needs,
        }
    };
    // A phi or select another far value or a call reads too would need both
    // copies: not followed.
    let mut refused: BTreeSet<ValueId> = BTreeSet::new();
    for (&value, &inst) in &chain {
        if matches!(
            function.instruction(inst).opcode,
            Opcode::Phi | Opcode::Select
        )
            && function.users(value).iter().any(|one| usage(function, value, one.user, one.index) == Reads::Needs)
        {
            refused.insert(value);
        }
    }
    // Optimistic: a value has the space its operands share until one differs; a
    // cycle of phis settles on its seeds.
    let mut state: BTreeMap<ValueId, Space> =
        chain.keys().map(|&one| (one, if refused.contains(&one) { Space::Neither } else { Space::Unknown })).collect();
    loop {
        let mut changed = false;
        for (&value, &inst) in &chain {
            let mut now = Space::Unknown;
            for operand in pointers(function, inst) {
                let there = match operand {
                    Operand::Value(one) => match seeds.get(&one) {
                        Some(&(_, space)) => Space::Near(space),
                        None => state.get(&one).copied().unwrap_or(Space::Neither),
                    },
                    _ => Space::Neither,
                };
                now = met(now, there);
            }
            if state[&value] == Space::Neither {
                continue;
            }
            if now != state[&value] && (state[&value] == Space::Unknown || now == Space::Neither) {
                state.insert(value, now);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let followed: BTreeMap<ValueId, u32> = state
        .iter()
        .filter_map(|(&value, &space)| if let Space::Near(one) = space { Some((value, one)) } else { None })
        .collect();
    let space_of =
        |value: ValueId| seeds.get(&value).map(|&(_, space)| space).or_else(|| followed.get(&value).copied());
    // Only what something reads through memory, or casts back, is worth a near
    // copy, with the values it is made of: a step only a compare reads
    // would be made, found dead and made again.
    let mut needed: Vec<ValueId> = followed
        .keys()
        .chain(seeds.keys())
        .copied()
        .filter(|&value| {
            function
                .users(value)
                .iter()
                .any(
                    |one| {
                        let reads = usage(function, value, one.user, one.index);
                        matches!(
                            function.instruction(one.user).opcode,
                            Opcode::Load { .. } | Opcode::Store { .. }
                        )
                            && reads == Reads::Narrows
                            || reads == Reads::Drops
                                && near(context, spaces, function.instruction(one.user).ty) == space_of(value)
                    },
                )
        })
        .collect();
    let mut wanted: BTreeSet<ValueId> = BTreeSet::new();
    while let Some(value) = needed.pop() {
        if !wanted.insert(value) {
            continue;
        }
        if let Some(&inst) = chain.get(&value) {
            for operand in pointers(function, inst) {
                if let Operand::Value(held) = operand {
                    needed.push(held);
                }
            }
        }
    }
    let inferred: Vec<(ValueId, InstId)> = chain
        .iter()
        .filter(|(value, _)| followed.contains_key(*value) && wanted.contains(*value))
        .map(|(&value, &inst)| (value, inst))
        .collect();
    // The near counterpart of each far value: a seed's is its source, a stack
    // object's made a stack pointer.
    let mut narrow: BTreeMap<ValueId, Operand> = BTreeMap::new();
    for (&value, &(source, space)) in &seeds {
        if !wanted.contains(&value) {
            continue;
        }
        let from = function.operand_type(context, source).and_then(|ty| near(context, spaces, ty));
        if from == Some(space) {
            narrow.insert(value, source);
        } else {
            // A stack object's address is a space-0 value until it is said to
            // be the stack's.
            let ValueDef::Instruction(def) = function.value(value).def else { continue };
            let made = function.create_instruction(
                Opcode::Cast(CastOp::AddrSpaceCast),
                near_types[&space],
                vec![source],
                Default::default(),
                None,
            );
            function.insert(made, Position::Before(def)).expect("placed");
            narrow.insert(value, Operand::Value(function.instruction(made).result.expect("a cast's value")));
        }
    }
    let mut shells = Vec::new();
    for &(value, inst) in &inferred {
        let instruction = function.instruction(inst).clone();
        if instruction.opcode == Opcode::Phi {
            let made = function.create_instruction(
                Opcode::Phi,
                near_types[&followed[&value]],
                Vec::new(),
                instruction.flags,
                None,
            );
            function.insert(made, Position::Before(inst)).expect("placed");
            narrow.insert(value, Operand::Value(function.instruction(made).result.expect("a phi's value")));
            shells.push((inst, made));
        }
    }
    // Steps and selects in program order: an operand's near value is made
    // before its user's.
    let mut remaining: Vec<(ValueId, InstId)> =
        inferred.iter().copied().filter(|&(_, inst)| function.instruction(inst).opcode != Opcode::Phi).collect();
    while !remaining.is_empty() {
        let before = remaining.len();
        let mut later = Vec::new();
        for (value, inst) in remaining {
            let instruction = function.instruction(inst).clone();
            let ready = pointers(function, inst)
                .iter()
                .all(|one| matches!(one, Operand::Value(held) if narrow.contains_key(held)));
            if !ready {
                later.push((value, inst));
                continue;
            }
            let mut operands = instruction.operands.clone();
            match instruction.opcode {
                Opcode::GetElementPtr { .. } => {
                    let Operand::Value(base) = operands[0] else { unreachable!("a far step's base") };
                    operands[0] = narrow[&base];
                }
                _ => {
                    for at in [1, 2] {
                        let Operand::Value(held) = operands[at] else { unreachable!("a far select's arm") };
                        operands[at] = narrow[&held];
                    }
                }
            }
            let made = function.create_instruction(
                instruction.opcode.clone(),
                near_types[&followed[&value]],
                operands,
                instruction.flags,
                None,
            );
            function.insert(made, Position::Before(inst)).expect("placed");
            narrow.insert(value, Operand::Value(function.instruction(made).result.expect("a value")));
        }
        remaining = later;
        if remaining.len() == before {
            break;
        }
    }
    // Phis last: every arm's near value exists.
    for (old, made) in shells {
        let operands: Vec<Operand> = function
            .instruction(old)
            .operands
            .chunks(2)
            .flat_map(|pair| {
                let Operand::Value(held) = pair[0] else { unreachable!("a far phi's arm") };
                [narrow[&held], pair[1]]
            })
            .collect();
        function.set_operands(made, operands);
    }
    // What reads a far value through memory, or casts it back, reads the near
    // one.
    let mut changed = false;
    let values: Vec<ValueId> = narrow.keys().copied().collect();
    for value in values {
        let near_value = narrow[&value];
        for one in function.users(value).snapshot() {
            match usage(function, value, one.user, one.index) {
                Reads::Narrows => {
                    if matches!(
                        function.instruction(one.user).opcode,
                        Opcode::Load { .. } | Opcode::Store { .. }
                    ) {
                        function.set_operand(one.user, one.index as usize, near_value);
                        changed = true;
                    }
                }
                Reads::Drops => {
                    // Only to the space it is: a cast to the other near space
                    // has no business here.
                    if near(context, spaces, function.instruction(one.user).ty) != space_of(value) {
                        continue;
                    }
                    let result = function.instruction(one.user).result.expect("a cast's value");
                    if !function.users(result).is_empty() {
                        function.replace_all_uses_with(result, near_value);
                        changed = true;
                    }
                }
                Reads::Needs => {}
            }
        }
    }
    changed
}
