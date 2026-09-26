//! What an instruction does to memory, as LLVM's `MemoryEffects` and
//! `Instruction::mayReadFromMemory`/`mayWriteToMemory` answer: a call from
//! its callee's `memory(...)`, anything else from its opcode.

use std::collections::HashMap;

use crate::context::{ConstantKind, Context, GlobalId};
use crate::module::{Function, GlobalKind, InstId, Module, Operand};
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

/// Each function's effects, as its attributes state them.
pub type Callees = HashMap<GlobalId, Effects>;

pub fn callees(module: &Module) -> Callees {
    module
        .globals
        .iter()
        .enumerate()
        .filter_map(|(at, global)| match &global.kind {
            GlobalKind::Function(function) => Some((GlobalId(at as u32), stated(&function.attrs))),
            GlobalKind::Variable(_) => None,
        })
        .collect()
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
            let callee = match instruction.operands.last() {
                Some(Operand::Constant(id)) => match context.get(*id).kind {
                    ConstantKind::Global(global) => callees.get(&global).copied(),
                    _ => None,
                },
                _ => None,
            };
            let at_site = stated(&info.attrs);
            let declared = callee.unwrap_or(Effects::ANY);
            Effects { reads: at_site.reads && declared.reads, writes: at_site.writes && declared.writes }
        }
        _ => Effects::NONE,
    }
}
