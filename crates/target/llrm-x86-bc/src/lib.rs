//! BC objects raised onto llrm-mir.
//!
//! One MIR function per body: `main`, each procedure by name. DGROUP is one
//! global per variable (`objects`), each runtime routine an `@llrm.qb.*`
//! declaration (`runtime`), each body's code its function (`emit`).
//! Refusal is fatal: `raise` answers the first function it cannot express,
//! and why.

pub mod access;
pub mod addresses;
pub mod arrays;
pub mod cells;
pub mod copies;
pub mod division;
pub mod emit;
pub mod floats;
pub mod longs;
pub mod machine;
pub mod objects;
pub mod pairs;
pub mod runtime;
pub mod sites;
pub mod tags;

use std::collections::BTreeMap;

pub use llrm_hir::mir::RUNTIME;

/// The selector-alone address space of `spaces`: BC's objects are real mode's,
/// which has one.
pub fn segment(spaces: &llrm_mir::spaces::Spaces) -> u32 {
    spaces.segment.expect("a BC object's target has selectors")
}

use llrm_mir::datalayout::DataLayout;
use llrm_mir::program::SegmentLayout;
use llrm_mir::{GlobalId, GlobalKind, Linkage, Module, Type};
use llrm_target::machine::Machine;
use llrm_x86_bcmachine::frontends::bc::blocks::has_header;
use llrm_x86_bcmachine::frontends::bc::extent::BodyKind;
use llrm_x86_bcmachine::objectfile::module::{self as found_module, Family};
use llrm_x86_bcmachine::objectfile::omf;

use crate::emit::Unit;
use crate::machine::{Answer, Facts, function_name};
use crate::objects::{Carving, Objects};

/// A function the raise cannot express, and why. `<module>` names one
/// that fails before any function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    pub function: String,
    pub reason: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}: {}", self.function, self.reason)
    }
}

pub const MODULE: &str = "<module>";

/// Each body raised, or why not; a refused one is left a declaration.
pub struct Raised {
    pub module: Module,
    /// The runtime it links against, declarations alone.
    pub runtime: Module,
    pub outcomes: Vec<(String, Result<(), String>)>,
    pub placement: Placement,
}

/// Where the object put what the raise made globals of, for a writer that
/// lays the data out again: each carved object's segment (by the object's
/// own index), first byte and global; and the segments, such as the code
/// segment, that data points into but that hold no object.
#[derive(Clone, Debug, Default)]
pub struct Placement {
    pub objects: Vec<(i64, i64, llrm_mir::GlobalId)>,
    pub bases: BTreeMap<i64, llrm_mir::GlobalId>,
}

impl Raised {
    pub fn refusals(&self) -> impl Iterator<Item = Refusal> + '_ {
        self.outcomes.iter().filter_map(|(function, outcome)| {
            outcome.as_ref().err().map(|reason| Refusal { function: function.clone(), reason: reason.clone() })
        })
    }
}

/// The module raised whole, as a program of its own, with its runtime and
/// where the object put its globals, or the first function refused.
pub fn raise(
    found: &found_module::Module,
    machine: &Machine,
) -> Result<Raised, Refusal> {
    raise_in(found, machine, &segments(machine, [found]))
}

/// `raise`, the module one of a program whose segments `segments` lays out.
pub fn raise_in(
    found: &found_module::Module,
    machine: &Machine,
    segments: &SegmentLayout,
) -> Result<Raised, Refusal> {
    let raised = raise_each_in(found, machine, segments)?;
    let refused = raised.refusals().next();
    match refused {
        Some(refusal) => Err(refusal),
        None => Ok(raised),
    }
}

/// The segments of the program `modules` are, as LINK lays them out: DGROUP
/// what any module's GRPDEF puts there, COMMON what any combines so, by
/// name; BC's code runs with SS = DS.
pub fn segments<'m>(
    machine: &Machine,
    modules: impl IntoIterator<Item = &'m found_module::Module>,
) -> SegmentLayout {
    let mut layout = SegmentLayout::of(&DataLayout::parse(&machine.layout().datalayout).expect("the target's layout"));
    for found in modules {
        let named = omf::segments(&found.records);
        let name = |index: &i64| named.get(*index as usize).cloned().flatten().map(|(name, _)| name);
        for member in found.dgroup.members.iter().filter_map(name) {
            if !layout.data_group.members.contains(&member) {
                layout.data_group.members.push(member);
            }
        }
        layout.data_group.common.extend(found.dgroup.shared.iter().filter_map(name));
    }
    layout
}

/// The bytes below BP the runtime's frame header takes.
pub fn header(family: Family) -> Option<i64> {
    match family {
        Family::Quickbasic => Some(10),
        Family::Pds => Some(18),
        Family::Vbdos => Some(20),
        _ => None,
    }
}

/// The main body's locals: where they start below BP, and how many bytes,
/// from the size the object's header records at code offset 0x22.
fn main_frame(found: &found_module::Module) -> Option<(i64, i64)> {
    let fixed = header(found_module::family(&found.records))?;
    if !has_header(found) {
        return None;
    }
    let size = i64::from(u16::from_le_bytes([
        found.code.get(0x22).copied().unwrap_or(0),
        found.code.get(0x23).copied().unwrap_or(0),
    ]));
    (0 < size && size < 0x8000 - fixed).then_some((-fixed - size, size))
}

/// Every body raised, each refusal recorded against its function.
pub fn raise_each(
    found: &found_module::Module,
    machine: &Machine,
) -> Result<Raised, Refusal> {
    raise_each_in(found, machine, &segments(machine, [found]))
}

/// `raise_each`, the module one of a program whose segments `segments`
/// lays out.
pub fn raise_each_in(
    found: &found_module::Module,
    machine: &Machine,
    segments: &SegmentLayout,
) -> Result<Raised, Refusal> {
    let module_refusal = |reason: String| Refusal { function: MODULE.to_owned(), reason };
    let facts = Facts::new(found, machine).map_err(module_refusal)?;
    let mut module = Module { datalayout: Some(machine.layout().datalayout.clone()), ..Module::default() };
    let mut intrinsics = BTreeMap::new();
    for name in ["uadd", "usub"] {
        for bits in [8, 16, 32] {
            let full = format!("llvm.{name}.with.overflow.i{bits}");
            let int = module.context.types.int(bits);
            let flag = module.context.types.int(1);
            let pair = module.context.types.intern(Type::Struct { fields: vec![int, flag], packed: false });
            let ty = module
                .context
                .types
                .intern(
                    Type::Function {
                        returns: pair,
                        parameters: vec![int, int],
                        variadic: false,
                    },
                );
            let global = module.add_function(&full, ty, Linkage::External).map_err(module_refusal)?;
            intrinsics.insert(full, (module.reference(global), ty));
        }
    }
    {
        let types = &mut module.context.types;
        let (void, pointer, byte, size, flag) = (types.void(), types.ptr(0), types.int(8), types.int(16), types.int(1));
        let ty = types.intern(Type::Function {
            returns: void,
            parameters: vec![pointer, byte, size, flag],
            variadic: false,
        });
        let global = module.add_function(emit::MEMSET, ty, Linkage::External).map_err(module_refusal)?;
        intrinsics.insert(emit::MEMSET.to_owned(), (module.reference(global), ty));
    }
    floats::declare(&facts, &mut module, &mut intrinsics).map_err(module_refusal)?;
    let mut procedures = BTreeMap::new();
    let mut functions = Vec::new();
    for (index, body) in facts.bodies.iter().enumerate() {
        // Each call to the event-poll adapter is B$EVCK's (emit).
        if body.body.kind == BodyKind::EventStub && facts.event_stub == Some(body.body.seed) {
            continue;
        }
        let name = function_name(&body.body);
        let (answer, popped) = match &body.interface {
            Some(Ok(interface)) => (interface.answer.clone(), interface.popped),
            _ => (Answer::None, 0),
        };
        let word = module.context.types.int(16);
        let returns = runtime::answer_type(&mut module, &answer);
        let ty = module
            .context
            .types
            .intern(
                Type::Function {
                    returns,
                    parameters: vec![word; (popped / 2) as usize],
                    variadic: false,
                },
            );
        let linkage = if body.body.kind == BodyKind::Procedure || body.body.kind == BodyKind::Main {
            Linkage::External
        } else {
            Linkage::Internal
        };
        let global = objects::add_unique(&mut module, &name, |module, one| module.add_function(one, ty, linkage));
        module.globals[global.0 as usize].address_space = facts.spaces.far;
        if body.body.kind == BodyKind::Procedure {
            let GlobalKind::Function(function) = &mut module.globals[global.0 as usize].kind else {
                unreachable!("a function")
            };
            function.calling_convention = llrm_mir::opcode::BASIC;
            let interface = body.interface.clone().unwrap_or_else(|| Err("no interface".to_owned()));
            procedures.insert(name.clone(), interface.map(|interface| (module.reference(global), ty, interface)));
        }
        functions.push((index, name, global));
    }
    let objects =
        Objects::build(&Carving::of(&facts, segments), found, &mut module, facts.spaces).map_err(module_refusal)?;
    let interfaces = runtime::interfaces(&facts);
    let callees = runtime::declare(&facts, &mut module, &interfaces);
    intrinsics.extend(access::declare(&facts, &mut module).map(|one| (access::declared(), one)));
    let family = found_module::family(&found.records);
    let err = if found.calls.values().any(|name| name == "B$FERR") {
        Some(llrm_hir::onerror::err(&mut module, facts.spaces.far).map_err(module_refusal)?)
    } else {
        None
    };
    let mut handled = BTreeMap::new();
    for &(index, _, global) in &functions {
        if facts.bodies[index].handler.is_some() {
            let lines: Vec<i64> = facts.statements.iter().map(|&(_, line)| line).collect();
            handled.insert(
                index,
                llrm_hir::onerror::handled(&mut module, facts.spaces.far, global, &lines, false)
                    .map_err(module_refusal)?,
            );
        }
    }
    // An intrinsic raises no BASIC error.
    let raising: Vec<GlobalId> = module
        .functions()
        .filter(|(_, global, function)| {
            !llrm_mir::facts::Facts::of(&function.attrs).no_unwind()
                && !global.name.as_deref().is_some_and(llrm_mir::intrinsics::is_reserved)
        })
        .map(|(id, _, _)| id)
        .collect();
    let raising = raising.into_iter().map(|id| module.reference(id)).collect();
    let unit = Unit {
        facts: &facts,
        objects: &objects,
        callees: &callees,
        procedures,
        intrinsics,
        main_frame: main_frame(found),
        header: header(family),
        raising,
        err,
    };
    let mut outcomes = Vec::new();
    for (index, name, global) in functions {
        let body = &facts.bodies[index];
        let outcome = {
            let mut builder = module.builder(global);
            emit::function(&mut builder, &unit, body, handled.get(&index).copied())
                .map(|()| addresses::attribute(builder.function, builder.context, &objects, &facts.spaces))
        };
        if outcome.is_err() {
            let GlobalKind::Function(function) = &mut module.globals[global.0 as usize].kind else {
                unreachable!("a function")
            };
            let declaration = function.declaration();
            *function = Box::new(declaration);
            module.globals[global.0 as usize].linkage = Linkage::External;
        }
        outcomes.push((name, outcome));
    }
    let runtime = cells::promise(&module, &facts, &objects).map_err(module_refusal)?;
    tags::tag(&mut module, crate::segment(&facts.spaces));
    let placement = objects.placement();
    Ok(Raised { module, runtime, outcomes, placement })
}
