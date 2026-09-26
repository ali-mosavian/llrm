//! What an instruction does to memory, as LLVM's `MemoryEffects` and
//! `Instruction::mayReadFromMemory`/`mayWriteToMemory` answer: a call from
//! its callee's `memory(...)`, anything else from its opcode.

use std::collections::HashMap;

use crate::context::{ConstantKind, Context, GlobalId};
use crate::datalayout::DataLayout;
use crate::module::{Function, GlobalKind, InstId, Module, Operand, ValueDef};
use crate::opcode::{Attribute, Opcode};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effects {
    pub reads: bool,
    pub writes: bool,
}

impl Effects {
    pub const NONE: Effects = Effects { reads: false, writes: false };
    pub const ANY: Effects = Effects { reads: true, writes: true };
}

/// What a call to a function does, as its attributes state it: its
/// effects on memory, and whether it always comes back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Summary {
    pub effects: Effects,
    /// Its effects only reach what its pointer arguments point to.
    pub arguments_only: bool,
    pub returns: bool,
}

pub type Callees = HashMap<GlobalId, Summary>;

pub fn callees(module: &Module) -> Callees {
    module
        .globals
        .iter()
        .enumerate()
        .filter_map(|(at, global)| match &global.kind {
            GlobalKind::Function(function) => Some((GlobalId(at as u32), summary(&function.attrs))),
            GlobalKind::Variable(_) => None,
        })
        .collect()
}

pub fn summary(attrs: &[Attribute]) -> Summary {
    Summary {
        effects: stated(attrs),
        arguments_only: argument_memory_only(attrs),
        returns: attrs.iter().any(|attr| matches!(attr, Attribute::Flag(flag) if flag == "willreturn")),
    }
}

/// Whether `attrs` confine every access to memory the pointer arguments
/// point to: `memory(argmem: ...)`.
pub fn argument_memory_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| match attr {
        Attribute::Memory(locations) => locations.iter().all(|(location, access)| access == "none" || location.as_deref() == Some("argmem")),
        _ => false,
    })
}

/// The function `inst` calls directly, if it is a call.
pub fn callee(context: &Context, function: &Function, inst: InstId) -> Option<GlobalId> {
    let instruction = function.instruction(inst);
    let (Opcode::Call(_) | Opcode::Invoke(_)) = instruction.opcode else { return None };
    match instruction.operands.last() {
        Some(Operand::Constant(id)) => match context.get(*id).kind {
            ConstantKind::Global(global) => Some(global),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `inst` is a call nothing needs: its value unread, touching no
/// memory, to a function that always comes back.
pub fn removable(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    let Opcode::Call(info) = &instruction.opcode else { return false };
    let returns = summary(&info.attrs).returns || callee(context, function, inst).and_then(|one| callees.get(&one)).is_some_and(|one| one.returns);
    returns && instruction.result.is_none_or(|result| function.users(result).is_empty()) && of(context, callees, function, inst) == Effects::NONE
}

/// What `memory(...)`, `readnone` or `readonly` among `attrs` allows.
pub fn stated(attrs: &[Attribute]) -> Effects {
    let mut effects = Effects::ANY;
    for attr in attrs {
        match attr {
            Attribute::Memory(locations) => {
                effects = Effects::NONE;
                for (_, access) in locations {
                    effects.reads |= access == "read" || access == "readwrite";
                    effects.writes |= access == "write" || access == "readwrite";
                }
            }
            Attribute::Flag(flag) if flag == "readnone" => effects = Effects::NONE,
            Attribute::Flag(flag) if flag == "readonly" => effects = Effects { reads: true, writes: false },
            _ => {}
        }
    }
    effects
}

/// What `inst` may do to memory.
pub fn of(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> Effects {
    let instruction = function.instruction(inst);
    match &instruction.opcode {
        Opcode::Load { volatile: false, .. } => Effects { reads: true, writes: false },
        Opcode::Store { volatile: false, .. } => Effects { reads: false, writes: true },
        // Volatile, as LLVM has it: ordered with every other access.
        Opcode::Load { .. } | Opcode::Store { .. } => Effects::ANY,
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let callee = callee(context, function, inst).and_then(|one| callees.get(&one)).map(|one| one.effects);
            let at_site = stated(&info.attrs);
            let declared = callee.unwrap_or(Effects::ANY);
            Effects { reads: at_site.reads && declared.reads, writes: at_site.writes && declared.writes }
        }
        _ => Effects::NONE,
    }
}

/// Whether nothing writes the memory `pointer` points into while the
/// function runs: it lies in a `noalias readonly` parameter's, as LLVM's
/// `getModRefInfoMask` finds.
pub fn invariant(context: &Context, layout: &DataLayout, function: &Function, pointer: Operand) -> bool {
    let (Operand::Value(base), _) = crate::valuetracking::underlying(context, layout, function, pointer) else { return false };
    let ValueDef::Argument(at) = function.value(base).def else { return false };
    let attrs = &function.parameter_attrs[at as usize];
    let has = |flag: &str| attrs.iter().any(|attr| matches!(attr, Attribute::Flag(one) if one == flag));
    has("noalias") && has("readonly")
}
