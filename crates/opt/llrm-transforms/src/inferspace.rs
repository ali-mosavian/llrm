//! LLVM's InferAddressSpaces: a far pointer made from a near one by `addrspacecast` addresses what
//! the near one does, so what only reads or writes through it, or steps it, can use the near one
//! and drop the selector: no `les`, no segment register held across a loop.
//!
//! ```text
//! %w = addrspacecast ptr %p to ptr addrspace(1)      %q = gep i8, ptr %p, 4
//! %g = gep i8, ptr addrspace(1) %w, 4           =>   load i16, ptr %q
//! load i16, ptr addrspace(1) %g
//! ```
//!
//! The near space is DGROUP's. A cast of a stack object stays far: the selector it makes is SS, and
//! a near pointer is read through DS. Phis and selects of such pointers follow, where every value
//! they join is one, and nothing else reads them: a far copy kept beside the near one for a call would
//! cost a register. A cast back to the near space, which the narrowing of a parameter leaves at each
//! call, is the near pointer itself.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::underlying;

/// The near space a far pointer is narrowed to: DGROUP's.
pub const NEAR_DATA: u32 = 0;

pub struct InferAddressSpaces;

impl FunctionPass for InferAddressSpaces {
    fn name(&self) -> &'static str {
        "inferspace"
    }

    fn run(&mut self, unit: &mut passes::Unit, _: &mut Analyses) -> PreservedAnalyses {
        // Blocks and edges are as they were.
        if inferred(unit.context, unit.layout, unit.function) { PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>() } else { PreservedAnalyses::all() }
    }
}

/// Whether `operand` is a stack object or steps from one: its far pointer addresses SS, not DS.
pub fn on_stack(context: &Context, layout: &DataLayout, function: &Function, operand: Operand) -> bool {
    let (Operand::Value(root), _) = underlying(context, layout, function, operand) else { return false };
    matches!(function.value(root).def, ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Alloca { .. }))
}

/// The far space `ty` is a pointer into, when narrowing may drop its selector.
fn far(context: &Context, layout: &DataLayout, ty: llrm_mir::types::TypeId) -> Option<u32> {
    match context.types.get(ty) {
        Type::Pointer(space) if layout.is_pair(*space) && !layout.carries(*space) => Some(*space),
        _ => None,
    }
}

fn near(context: &Context, ty: llrm_mir::types::TypeId) -> bool {
    matches!(context.types.get(ty), Type::Pointer(space) if *space == NEAR_DATA)
}

/// Where `v`'s value goes: how each of its users reads it.
#[derive(PartialEq)]
enum Reads {
    /// The pointer of a load or store, or the base of a step: it can read the near one.
    Narrows,
    /// A cast back to the near space.
    Drops,
    /// Anything that needs the selector.
    Needs,
}

fn inferred(context: &mut Context, layout: &DataLayout, function: &mut Function) -> bool {
    let near_ty = context.types.ptr(NEAR_DATA);
    let context: &Context = context;
    let walk: Vec<(llrm_mir::module::BlockId, InstId)> = function.walk().collect();
    // The far pointers that are a near one cast: value -> the near pointer.
    let mut seeds: BTreeMap<ValueId, Operand> = BTreeMap::new();
    for &(_, inst) in &walk {
        let instruction = function.instruction(inst);
        let (Opcode::Cast(CastOp::AddrSpaceCast), Some(result)) = (&instruction.opcode, instruction.result) else { continue };
        let source = instruction.operands[0];
        let Some(from) = function.operand_type(context, source) else { continue };
        if far(context, layout, instruction.ty).is_some() && near(context, from) && !on_stack(context, layout, function, source) {
            seeds.insert(result, source);
        }
    }
    if seeds.is_empty() {
        return false;
    }
    // Far steps, phis and selects that may follow: by value.
    let mut chain: BTreeMap<ValueId, InstId> = BTreeMap::new();
    for &(_, inst) in &walk {
        let instruction = function.instruction(inst);
        if matches!(instruction.opcode, Opcode::GetElementPtr { .. } | Opcode::Phi | Opcode::Select) {
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
            Opcode::Phi | Opcode::Select if instruction.result.is_some_and(|one| chain.contains_key(&one)) => Reads::Narrows,
            Opcode::Cast(CastOp::AddrSpaceCast) if near(context, instruction.ty) && function.value(value).ty != instruction.ty => Reads::Drops,
            _ => Reads::Needs,
        }
    };
    // A phi or select another far value or a call reads too would need both copies: not followed.
    let mut refused: BTreeSet<ValueId> = BTreeSet::new();
    for (&value, &inst) in &chain {
        if matches!(function.instruction(inst).opcode, Opcode::Phi | Opcode::Select) && function.users(value).iter().any(|one| usage(function, value, one.user, one.index) == Reads::Needs) {
            refused.insert(value);
        }
    }
    // Optimistic: a value is near until an operand is not; a cycle of phis settles on its seeds.
    let mut state: BTreeMap<ValueId, Option<()>> = chain.keys().filter(|one| !refused.contains(*one)).map(|&one| (one, Some(()))).collect();
    loop {
        let mut changed = false;
        for (&value, &inst) in &chain {
            if state.get(&value) != Some(&Some(())) {
                continue;
            }
            let near_operand = |operand: Operand, state: &BTreeMap<ValueId, Option<()>>| match operand {
                Operand::Value(one) => seeds.contains_key(&one) || state.get(&one) == Some(&Some(())),
                _ => false,
            };
            if !pointers(function, inst).into_iter().all(|one| near_operand(one, &state)) {
                state.insert(value, None);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let followed: BTreeSet<ValueId> = chain.keys().copied().filter(|value| state.get(value) == Some(&Some(()))).collect();
    // Only what something reads through memory, or casts back, is worth a near copy, with the values
    // it is made of: a step only a compare reads would be made, found dead and made again.
    let mut needed: Vec<ValueId> = followed
        .iter()
        .chain(seeds.keys())
        .copied()
        .filter(|&value| function.users(value).iter().any(|one| matches!(function.instruction(one.user).opcode, Opcode::Load { .. } | Opcode::Store { .. } | Opcode::Cast(_)) && usage(function, value, one.user, one.index) != Reads::Needs))
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
    let inferred: Vec<(ValueId, InstId)> = chain.iter().filter(|(value, _)| followed.contains(value) && wanted.contains(value)).map(|(&value, &inst)| (value, inst)).collect();
    // The near counterpart of each far value.
    let mut narrow: BTreeMap<ValueId, Operand> = seeds.iter().map(|(&value, &source)| (value, source)).collect();
    let mut shells = Vec::new();
    for &(value, inst) in &inferred {
        let instruction = function.instruction(inst).clone();
        match instruction.opcode {
            Opcode::Phi => {
                let made = function.create_instruction(Opcode::Phi, near_ty, Vec::new(), instruction.flags, None);
                function.insert(made, Position::Before(inst)).expect("placed");
                narrow.insert(value, Operand::Value(function.instruction(made).result.expect("a phi's value")));
                shells.push((inst, made));
            }
            _ => {}
        }
    }
    // Steps and selects in program order: an operand's near value is made before its user's.
    let mut remaining: Vec<(ValueId, InstId)> = inferred.iter().copied().filter(|&(_, inst)| function.instruction(inst).opcode != Opcode::Phi).collect();
    while !remaining.is_empty() {
        let before = remaining.len();
        let mut later = Vec::new();
        for (value, inst) in remaining {
            let instruction = function.instruction(inst).clone();
            let ready = pointers(function, inst).iter().all(|one| matches!(one, Operand::Value(held) if narrow.contains_key(held)));
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
            let made = function.create_instruction(instruction.opcode.clone(), near_ty, operands, instruction.flags, None);
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
    // What reads a far value through memory, or casts it back, reads the near one.
    let mut changed = false;
    let values: Vec<ValueId> = narrow.keys().copied().collect();
    for value in values {
        let near_value = narrow[&value];
        for one in function.users(value).to_vec() {
            match usage(function, value, one.user, one.index) {
                Reads::Narrows => {
                    if matches!(function.instruction(one.user).opcode, Opcode::Load { .. } | Opcode::Store { .. }) {
                        function.set_operand(one.user, one.index as usize, near_value);
                        changed = true;
                    }
                }
                Reads::Drops => {
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
