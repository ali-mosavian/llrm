//! What an instruction does to memory, as LLVM's `MemoryEffects` and
//! `Instruction::mayReadFromMemory`/`mayWriteToMemory` answer: a call from
//! its callee's `memory(...)`, anything else from its opcode.

use crate::context::{ConstantKind, Context, GlobalId};
use crate::datalayout::DataLayout;
use crate::facts::{Fact, Facts};
use crate::hash::HashMap;
use crate::module::{Function, GlobalKind, GlobalValue, InstId, Module, Operand, ValueDef};
use crate::opcode::{Attribute, BinaryOp, Opcode};

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
    /// It has a body: `!isDeclaration()`, for a function pass that may not read
    /// another function.
    pub defined: bool,
    pub effects: Effects,
    /// Those of `effects` on memory the program can name: not
    /// `inaccessiblemem`.
    pub accessible: Effects,
    /// Its effects only reach what its pointer arguments point to.
    pub arguments_only: bool,
    pub returns: bool,
    /// Each parameter it keeps no copy of.
    pub nocapture: Vec<bool>,
    /// It is `llvm.memset`: its first argument's bytes, as many as the
    /// third says, become the second.
    pub memset: bool,
    /// It is `llvm.experimental.memset.pattern`: as `memset`, of cells
    /// of the second argument's width, counted by the third.
    pub pattern: bool,
    /// It is `llvm.lifetime.start` or `.end`: the object its second argument
    /// points to has its bytes live, or not, from here.
    pub lifetime: bool,
    /// It is `llvm.expect`: its first argument, with a hint of what it is
    /// expected to be.
    pub expect: bool,
}

/// Hashed with fixed keys: std's seeds each map afresh, and the work of
/// building one then differs run to run (`callee-effects` read 97.7 to 98.6
/// Minstr compiling one program, which put a step at the scaling gate's 2%
/// floor in and out of its budget).
pub type Callees = HashMap<GlobalId, Summary>;

/// One scan of every function in `module`; counted as `callees` for an
/// observer, which a pass that asks per query shows.
pub fn callees(module: &Module) -> Callees {
    crate::passes::counted("callees", false);
    module
        .globals
        .iter()
        .enumerate()
        .filter_map(|(at, global)| match &global.kind {
            GlobalKind::Function(function) => {
                Some((GlobalId(at as u32), global_summary(global.name.as_deref(), function)))
            }
            GlobalKind::Variable(_) => None,
        })
        .collect()
}

/// What a function named `name` does to memory: its attributes, and the
/// intrinsic its name says it is. A declaration made during a pass run gets
/// its summary the same way the module's do.
pub fn global_summary(
    name: Option<&str>,
    function: &Function,
) -> Summary {
    let named = |prefix: &str| name.is_some_and(|name| name.starts_with(prefix));
    Summary {
        memset: named("llvm.memset."),
        pattern: named("llvm.experimental.memset.pattern."),
        lifetime: named("llvm.lifetime."),
        expect: named("llvm.expect."),
        ..summary(function)
    }
}

pub fn summary(function: &Function) -> Summary {
    let attrs = &function.attrs;
    Summary {
        defined: !function.is_declaration(),
        effects: stated(attrs),
        accessible: stated_at(attrs, |location| location != Some("inaccessiblemem")),
        arguments_only: argument_memory_only(attrs),
        returns: returns(attrs),
        nocapture: function.parameter_attrs.iter().map(|one| Fact::NoCapture.stated_by(one)).collect(),
        memset: false,
        pattern: false,
        lifetime: false,
        expect: false,
    }
}

/// A call to `llvm.experimental.memset.pattern`: where, the cell, and how many
/// cells.
pub fn pattern(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> Option<(Operand, Operand, Operand)> {
    let summary = callees.get(&callee(context, function, inst)?)?;
    let operands = &function.instruction(inst).operands;
    (summary.pattern && operands.len() == 5).then(|| (operands[0], operands[1], operands[2]))
}

/// A call to `llvm.memset`: where, the byte, and how many.
pub fn memset(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> Option<(Operand, Operand, Operand)> {
    let summary = callees.get(&callee(context, function, inst)?)?;
    let operands = &function.instruction(inst).operands;
    (summary.memset && operands.len() == 5).then(|| (operands[0], operands[1], operands[2]))
}

/// A call to `llvm.lifetime.start` or `.end`: the pointer it is about.
pub fn lifetime(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> Option<Operand> {
    let summary = callees.get(&callee(context, function, inst)?)?;
    let operands = &function.instruction(inst).operands;
    (summary.lifetime && operands.len() == 3).then(|| operands[1])
}

/// Whether `function` calls a routine that returns twice (`setjmp`): the call
/// or its callee says so. In such a function a stack slot another local used
/// before the first return holds that local's value after the second, so no
/// slot is shared.
pub fn calls_returns_twice(
    module: &Module,
    function: &Function,
) -> bool {
    function.walk().any(|(_, inst)| {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return false };
        has(&info.attrs, "returns_twice")
            || callee(&module.context, function, inst)
                .and_then(|one| module.global(one).function())
                .is_some_and(|one| has(&one.attrs, "returns_twice"))
    })
}

/// Whether `attrs` carry the flag `flag`: for what is no fact (`returns_twice`,
/// `optnone`); a fact is asked of `Facts`.
pub fn has(
    attrs: &[Attribute],
    flag: &str,
) -> bool {
    attrs.iter().any(|attr| matches!(attr, Attribute::Flag(one) if one == flag))
}

/// Whether `attrs` promise `willreturn`.
pub fn returns(attrs: &[Attribute]) -> bool {
    Fact::WillReturn.stated_by(attrs)
}

/// Whether the call `inst` keeps no copy of its argument `index`.
pub fn nocapture(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
    index: usize,
) -> bool {
    let Opcode::Call(info) = &function.instruction(inst).opcode else { return false };
    info.argument_attrs.get(index).is_some_and(|attrs| Fact::NoCapture.stated_by(attrs))
        || callee(context, function, inst)
            .and_then(|one| callees.get(&one))
            .is_some_and(|one| one.nocapture.get(index) == Some(&true))
}

/// Whether the call `inst` keeps neither its argument `index` nor any pointer
/// read out of what that points to: `noretain` at the site or on the callee
/// among `globals`.
pub fn noretain(
    context: &Context,
    globals: &[GlobalValue],
    function: &Function,
    inst: InstId,
    index: usize,
) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return false };
    let declared =
        callee(context, function, inst).and_then(|one| globals.get(one.0 as usize)).and_then(|one| one.function());
    info.argument_attrs.get(index).is_some_and(|attrs| Fact::NoRetain.stated_by(attrs))
        || declared.and_then(|one| one.parameter_attrs.get(index)).is_some_and(|attrs| Fact::NoRetain.stated_by(attrs))
}

/// Whether `attrs` confine every access to memory the pointer arguments
/// point to: `memory(argmem: ...)`.
pub fn argument_memory_only(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(
            |attr| match attr {
                Attribute::Memory(locations) => {
                    locations.iter().all(|(location, access)| access == "none" || location.as_deref() == Some("argmem"))
                }
                _ => false,
            },
        )
}

/// The function `inst` calls directly, if it is a call.
#[inline]
pub fn callee(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<GlobalId> {
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

/// `isNoAliasCall`: whether the call `inst` returns `noalias`, as the call
/// site or its callee states it (`CallBase::hasRetAttr` reads both).
pub fn returns_noalias(
    context: &Context,
    globals: &[GlobalValue],
    function: &Function,
    inst: InstId,
) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return false };
    Fact::NoAlias.stated_by(&info.return_attrs)
        || callee(context, function, inst).and_then(|global| globals.get(global.0 as usize)).is_some_and(|global| {
            match &global.kind {
                GlobalKind::Function(callee) => Fact::NoAlias.stated_by(&callee.return_attrs),
                GlobalKind::Variable(_) => false,
            }
        })
}

/// Whether `inst`'s only effect is its value, so that if nothing needs the
/// value it may go: a pure operation, a plain load, or a call touching no
/// memory to a function that always comes back.
pub fn only_value(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> bool {
    match function.instruction(inst).opcode {
        Opcode::Call(_) => {
            call_returns(context, callees, function, inst) && of(context, callees, function, inst) == Effects::NONE
        }
        ref opcode => pure_operation(opcode),
    }
}

/// Work that stores nothing, reads nothing and cannot trap: `only_value` less
/// loads, allocas, calls and the divisions, which trap.
pub fn speculatable(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> bool {
    let op = function.instruction(inst);
    let traps = matches!(
        op.opcode,
        Opcode::Binary(BinaryOp::SDiv | BinaryOp::UDiv | BinaryOp::SRem | BinaryOp::URem)
    );
    let reads = matches!(
        op.opcode,
        Opcode::Load { .. } | Opcode::Alloca { .. } | Opcode::Call(_)
    );
    !traps && !reads && only_value(context, callees, function, inst)
}

/// Whether `opcode` is an operation, a plain load among them, whose only effect
/// is its value.
pub fn pure_operation(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Binary(_)
            | Opcode::Cast(_)
            | Opcode::ICmp(_)
            | Opcode::FCmp(_)
            | Opcode::GetElementPtr { .. }
            | Opcode::Phi
            | Opcode::Select
            | Opcode::FNeg
            | Opcode::ExtractValue(_)
            | Opcode::InsertValue(_)
            | Opcode::Freeze
            | Opcode::Alloca { .. }
            | Opcode::Load { volatile: false, .. }
    )
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
pub fn stated_at(
    attrs: &[Attribute],
    counted: impl Fn(Option<&str>) -> bool,
) -> Effects {
    LOCATIONS
        .into_iter()
        .filter(|&one| counted(one))
        .map(|one| at(attrs, one))
        .fold(
            Effects::NONE,
            |one, other| Effects { reads: one.reads || other.reads, writes: one.writes || other.writes },
        )
}

/// What `attrs` allow on the memory a call's pointer arguments point to,
/// and on every other location the module sees, as LLVM's `MemoryEffects`
/// keeps them per location. `inaccessiblemem` is neither.
pub fn located(attrs: &[Attribute]) -> (Effects, Effects) {
    (at(attrs, Some("argmem")), at(attrs, None))
}

/// Where a rewrite moved a function's accesses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Moved {
    /// Memory reached through a loaded pointer is now reached through a
    /// parameter (argument promotion).
    IntoArguments,
    /// Memory reached through a parameter is now reached otherwise (a
    /// parameter replaced by a constant address, a far pointer by an offset).
    OutOfArguments,
}

/// `attrs`' `memory(...)` after a rewrite that moved accesses between argument
/// memory and every other location: the destination is allowed what the source
/// was, since the split the attribute stated no longer says where the body
/// reads and writes.
pub fn unsplit(
    attrs: &mut Vec<Attribute>,
    moved: Moved,
) {
    let (arguments, other) = located(attrs);
    if !attrs.iter().any(|one| matches!(one, Attribute::Memory(_))) {
        return;
    }
    let join = |one: Effects, two: Effects| Effects { reads: one.reads || two.reads, writes: one.writes || two.writes };
    let (arguments, other) = match moved {
        Moved::IntoArguments => (join(arguments, other), other),
        Moved::OutOfArguments => (arguments, join(arguments, other)),
    };
    let (kept_arguments, kept_other) = located(attrs);
    if (arguments, other) == (kept_arguments, kept_other) {
        return;
    }
    let access = |one: Effects| {
        match (one.reads, one.writes) {
            (true, true) => "readwrite",
            (true, false) => "read",
            (false, true) => "write",
            (false, false) => "none",
        }
        .to_owned()
    };
    let hidden = inaccessible(attrs);
    let mut locations = vec![(None, access(other))];
    if arguments != other {
        locations.push((Some("argmem".to_owned()), access(arguments)));
    }
    if hidden != other {
        locations.push((Some("inaccessiblemem".to_owned()), access(hidden)));
    }
    attrs.retain(|one| !matches!(one, Attribute::Memory(_)));
    attrs.push(Attribute::Memory(locations));
}

/// What `attrs` allow on memory no pointer of the module reaches:
/// `inaccessiblemem`.
pub fn inaccessible(attrs: &[Attribute]) -> Effects {
    at(attrs, Some("inaccessiblemem"))
}

/// What `attrs` allow on one of `LOCATIONS`.
fn at(
    attrs: &[Attribute],
    location: Option<&str>,
) -> Effects {
    let access = |one: &str| Effects {
        reads: one == "read" || one == "readwrite",
        writes: one == "write" || one == "readwrite",
    };
    let mut effects = Effects::ANY;
    for attr in attrs {
        let stated = match attr {
            Attribute::Memory(locations) => {
                let default =
                    locations.iter().find(|(one, _)| one.is_none()).map_or(Effects::NONE, |(_, one)| access(one));
                match location {
                    Some(_) => locations
                        .iter()
                        .find(|(one, _)| one.as_deref() == location)
                        .map_or(default, |(_, one)| access(one)),
                    // Another named location, as `errnomem`, is part of the
                    // rest.
                    None => locations
                        .iter()
                        .filter(|(one, _)| one.is_some() && !LOCATIONS.contains(&one.as_deref()))
                        .fold(default, |effects, (_, one)| {
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
    // Only the three flags matter: `Facts::of` over the whole list built a
    // vector of every fact for each of 12.5 K calls in d_faces.
    let stated = |fact: Fact| fact.stated_by(attrs);
    let mut effects = if stated(Fact::ReadNone) { Effects::NONE } else { Effects::ANY };
    effects.writes &= !stated(Fact::ReadOnly);
    effects.reads &= !stated(Fact::WriteOnly);
    effects
}

/// The byte ranges `initializes` among `attrs` says are written before
/// anything reads them.
pub fn initializes(attrs: &[Attribute]) -> &[(i64, i64)] {
    attrs
        .iter()
        .find_map(|attr| if let Attribute::Initializes(ranges) = attr { Some(ranges.as_slice()) } else { None })
        .unwrap_or_default()
}

/// What `inst` may do to memory the program can name: a call's effects
/// less those on `inaccessiblemem`, where a routine that ends the program
/// keeps its own.
pub fn accessible(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> Effects {
    let instruction = function.instruction(inst);
    match &instruction.opcode {
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let at_site = stated_at(&info.attrs, |location| location != Some("inaccessiblemem"));
            let declared = callee(context, function, inst)
                .and_then(|one| callees.get(&one))
                .map_or(Effects::ANY, |one| one.accessible);
            Effects { reads: at_site.reads && declared.reads, writes: at_site.writes && declared.writes }
        }
        _ => of(context, callees, function, inst),
    }
}

/// What `inst` may do to memory.
pub fn of(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> Effects {
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
pub fn invariant(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    pointer: Operand,
) -> bool {
    let (Operand::Value(base), _) = crate::valuetracking::underlying(context, layout, function, pointer) else {
        return false;
    };
    let ValueDef::Argument(at) = function.value(base).def else { return false };
    let attrs = &function.parameter_attrs[at as usize];
    let facts = Facts::of(attrs);
    facts.no_alias() && facts.read_only()
}

/// Whether the load `inst` reads memory nothing writes once it is initialised:
/// the language says so of the load (`!invariant.load`) or of what it reads
/// (`invariant`). Every pass that asks what may clobber a load asks this first.
pub fn invariant_load(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    matches!(instruction.opcode, Opcode::Load { volatile: false, .. })
        && (Facts::of_instruction(function, inst).invariant()
            || invariant(context, layout, function, instruction.operands[0]))
}

/// Whether the call `inst` always comes back, as it or its callee says.
pub fn call_returns(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> bool {
    let Opcode::Call(info) = &function.instruction(inst).opcode else { return false };
    returns(&info.attrs)
        || callee(context, function, inst).and_then(|one| callees.get(&one)).is_some_and(|one| one.returns)
}

#[cfg(test)]
mod stated_tests {
    use super::*;

    fn flag(name: &str) -> Attribute {
        Attribute::Flag(name.to_owned())
    }

    /// `through`, `returns` and the single-flag asks read the flags off the
    /// list; they built every fact of it first (12.5 K calls, 3 M Ir in
    /// d_faces). Each must still answer as the facts do, whatever else the
    /// list holds.
    #[test]
    fn a_flag_asked_alone_is_the_fact_the_whole_list_states() {
        let lists = [
            vec![],
            vec![flag("readnone")],
            vec![flag("readonly"), flag("nounwind")],
            vec![flag("writeonly"), Attribute::Int("dereferenceable".to_owned(), 8)],
            vec![flag("readonly"), flag("writeonly"), flag("willreturn")],
            vec![Attribute::Memory(vec![(None, "read".to_owned())]), flag("noinline"), flag("nocapture")],
            vec![flag("noalias"), flag("noretain"), Attribute::Str("willreturn".to_owned(), None)],
        ];
        for attrs in &lists {
            let facts = Facts::of(attrs);
            let mut want = if facts.read_none() { Effects::NONE } else { Effects::ANY };
            want.writes &= !facts.read_only();
            want.reads &= !facts.write_only();
            assert_eq!(through(attrs), want, "{attrs:?}");
            assert_eq!(returns(attrs), facts.will_return(), "{attrs:?}");
            assert_eq!(Fact::NoCapture.stated_by(attrs), facts.no_capture(), "{attrs:?}");
            assert_eq!(Fact::NoRetain.stated_by(attrs), facts.no_retain(), "{attrs:?}");
            assert_eq!(Fact::NoAlias.stated_by(attrs), facts.no_alias(), "{attrs:?}");
        }
    }
}
