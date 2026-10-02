//! What an instruction does to memory, as LLVM's `MemoryEffects` and
//! `Instruction::mayReadFromMemory`/`mayWriteToMemory` answer: a call from
//! its callee's `memory(...)`, anything else from its opcode.

use std::collections::HashMap;

use crate::context::{ConstantKind, Context, GlobalId};
use crate::facts::Facts;
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Summary {
    pub effects: Effects,
    /// Those of `effects` on memory the program can name: not `inaccessiblemem`.
    pub accessible: Effects,
    /// Its effects only reach what its pointer arguments point to.
    pub arguments_only: bool,
    pub returns: bool,
    /// Each parameter it keeps no copy of.
    pub nocapture: Vec<bool>,
    /// It is `llvm.memset`: its first argument's bytes, as many as the
    /// third says, become the second.
    pub memset: bool,
    /// It is `llvm.lifetime.start` or `.end`: the object its second argument
    /// points to has its bytes live, or not, from here.
    pub lifetime: bool,
}

pub type Callees = HashMap<GlobalId, Summary>;

pub fn callees(module: &Module) -> Callees {
    module
        .globals
        .iter()
        .enumerate()
        .filter_map(|(at, global)| match &global.kind {
            GlobalKind::Function(function) => {
                let named = |prefix: &str| global.name.as_deref().is_some_and(|name| name.starts_with(prefix));
                Some((GlobalId(at as u32), Summary { memset: named("llvm.memset."), lifetime: named("llvm.lifetime."), ..summary(function) }))
            }
            GlobalKind::Variable(_) => None,
        })
        .collect()
}

pub fn summary(function: &Function) -> Summary {
    let attrs = &function.attrs;
    Summary {
        effects: stated(attrs),
        accessible: stated_at(attrs, |location| location != Some("inaccessiblemem")),
        arguments_only: argument_memory_only(attrs),
        returns: returns(attrs),
        nocapture: function.parameter_attrs.iter().map(|one| Facts::of(one).no_capture()).collect(),
        memset: false,
        lifetime: false,
    }
}

/// A call to `llvm.memset`: where, the byte, and how many.
pub fn memset(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> Option<(Operand, Operand, Operand)> {
    let summary = callees.get(&callee(context, function, inst)?)?;
    let operands = &function.instruction(inst).operands;
    (summary.memset && operands.len() == 5).then(|| (operands[0], operands[1], operands[2]))
}

/// A call to `llvm.lifetime.start` or `.end`: the pointer it is about.
pub fn lifetime(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> Option<Operand> {
    let summary = callees.get(&callee(context, function, inst)?)?;
    let operands = &function.instruction(inst).operands;
    (summary.lifetime && operands.len() == 3).then(|| operands[1])
}

/// Whether `function` calls a routine that returns twice (`setjmp`): the call or
/// its callee says so. In such a function a stack slot another local used before the
/// first return holds that local's value after the second, so no slot is shared.
pub fn calls_returns_twice(module: &Module, function: &Function) -> bool {
    function.walk().any(|(_, inst)| {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return false };
        has(&info.attrs, "returns_twice") || callee(&module.context, function, inst).and_then(|one| module.global(one).function()).is_some_and(|one| has(&one.attrs, "returns_twice"))
    })
}

/// Whether `attrs` carry the flag `flag`: for what is no fact (`returns_twice`,
/// `optnone`); a fact is asked of `Facts`.
pub fn has(attrs: &[Attribute], flag: &str) -> bool {
    attrs.iter().any(|attr| matches!(attr, Attribute::Flag(one) if one == flag))
}

/// Whether `attrs` promise `willreturn`.
pub fn returns(attrs: &[Attribute]) -> bool {
    Facts::of(attrs).will_return()
}

/// Whether the call `inst` keeps no copy of its argument `index`.
pub fn nocapture(context: &Context, callees: &Callees, function: &Function, inst: InstId, index: usize) -> bool {
    let Opcode::Call(info) = &function.instruction(inst).opcode else { return false };
    info.argument_attrs.get(index).is_some_and(|attrs| Facts::of(attrs).no_capture())
        || callee(context, function, inst).and_then(|one| callees.get(&one)).is_some_and(|one| one.nocapture.get(index) == Some(&true))
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

/// Whether `inst`'s only effect is its value, so that if nothing needs the
/// value it may go: a pure operation, a plain load, or a call touching no
/// memory to a function that always comes back.
pub fn only_value(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> bool {
    match function.instruction(inst).opcode {
        Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::GetElementPtr { .. } | Opcode::Phi | Opcode::Select
        | Opcode::FNeg | Opcode::ExtractValue(_) | Opcode::InsertValue(_) | Opcode::Freeze | Opcode::Alloca { .. } | Opcode::Load { volatile: false, .. } => true,
        Opcode::Call(_) => call_returns(context, callees, function, inst) && of(context, callees, function, inst) == Effects::NONE,
        _ => false,
    }
}

/// What `memory(...)`, `readnone`, `readonly` or `writeonly` among `attrs`
/// allows.
pub fn stated(attrs: &[Attribute]) -> Effects {
    stated_at(attrs, |_| true)
}

/// The locations `memory(...)` tells apart: what the pointer arguments
/// point to, memory no pointer of the module reaches, and every other
/// (`None`).
const LOCATIONS: [Option<&str>; 3] = [Some("argmem"), Some("inaccessiblemem"), None];

/// What `attrs` allow at the locations `counted` admits (`None` is every
/// one but `argmem` and `inaccessiblemem`).
pub fn stated_at(attrs: &[Attribute], counted: impl Fn(Option<&str>) -> bool) -> Effects {
    LOCATIONS.into_iter().filter(|&one| counted(one)).map(|one| at(attrs, one)).fold(Effects::NONE, |one, other| Effects { reads: one.reads || other.reads, writes: one.writes || other.writes })
}

/// What `attrs` allow on the memory a call's pointer arguments point to,
/// and on every other location the module sees, as LLVM's `MemoryEffects`
/// keeps them per location. `inaccessiblemem` is neither.
pub fn located(attrs: &[Attribute]) -> (Effects, Effects) {
    (at(attrs, Some("argmem")), at(attrs, None))
}

/// What `attrs` allow on memory no pointer of the module reaches:
/// `inaccessiblemem`.
pub fn inaccessible(attrs: &[Attribute]) -> Effects {
    at(attrs, Some("inaccessiblemem"))
}

/// What `attrs` allow on one of `LOCATIONS`.
fn at(attrs: &[Attribute], location: Option<&str>) -> Effects {
    let access = |one: &str| Effects { reads: one == "read" || one == "readwrite", writes: one == "write" || one == "readwrite" };
    let mut effects = Effects::ANY;
    for attr in attrs {
        let stated = match attr {
            Attribute::Memory(locations) => {
                let default = locations.iter().find(|(one, _)| one.is_none()).map_or(Effects::NONE, |(_, one)| access(one));
                match location {
                    Some(_) => locations.iter().find(|(one, _)| one.as_deref() == location).map_or(default, |(_, one)| access(one)),
                    // Another named location, as `errnomem`, is part of the rest.
                    None => locations.iter().filter(|(one, _)| one.is_some() && !LOCATIONS.contains(&one.as_deref())).fold(default, |effects, (_, one)| {
                        let one = access(one);
                        Effects { reads: effects.reads || one.reads, writes: effects.writes || one.writes }
                    }),
                }
            }
            Attribute::Flag(_) => through(std::slice::from_ref(attr)),
            _ => continue,
        };
        effects = Effects { reads: effects.reads && stated.reads, writes: effects.writes && stated.writes };
    }
    effects
}

/// What `readnone`, `readonly` or `writeonly` among `attrs` allow, on a
/// function or on one pointer parameter.
pub fn through(attrs: &[Attribute]) -> Effects {
    let facts = Facts::of(attrs);
    let mut effects = if facts.read_none() { Effects::NONE } else { Effects::ANY };
    effects.writes &= !facts.read_only();
    effects.reads &= !facts.write_only();
    effects
}

/// The byte ranges `initializes` among `attrs` says are written before
/// anything reads them.
pub fn initializes(attrs: &[Attribute]) -> &[(i64, i64)] {
    attrs.iter().find_map(|attr| if let Attribute::Initializes(ranges) = attr { Some(ranges.as_slice()) } else { None }).unwrap_or_default()
}

/// What `inst` may do to memory the program can name: a call's effects
/// less those on `inaccessiblemem`, where a routine that ends the program
/// keeps its own.
pub fn accessible(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> Effects {
    let instruction = function.instruction(inst);
    match &instruction.opcode {
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let at_site = stated_at(&info.attrs, |location| location != Some("inaccessiblemem"));
            let declared = callee(context, function, inst).and_then(|one| callees.get(&one)).map_or(Effects::ANY, |one| one.accessible);
            Effects { reads: at_site.reads && declared.reads, writes: at_site.writes && declared.writes }
        }
        _ => of(context, callees, function, inst),
    }
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
    let facts = Facts::of(attrs);
    facts.no_alias() && facts.read_only()
}

/// Whether the call `inst` always comes back, as it or its callee says.
pub fn call_returns(context: &Context, callees: &Callees, function: &Function, inst: InstId) -> bool {
    let Opcode::Call(info) = &function.instruction(inst).opcode else { return false };
    returns(&info.attrs) || callee(context, function, inst).and_then(|one| callees.get(&one)).is_some_and(|one| one.returns)
}
