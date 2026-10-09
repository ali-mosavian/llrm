//! What each call's callee is in MIR: a runtime routine declared as
//! `@llrm.qb.<name>` with what its contract promises, or a procedure of the
//! module.
//!
//! A routine's interface: its register inputs first, in
//! `runtime::direct_slots` order, then its stack arguments as words. A
//! callee that pops them is BASIC's convention, first pushed first; one
//! that leaves them to the caller is C's, last pushed first. It answers in AX (`i16`), DX:AX
//! (`i32`), or several registers as a struct in slot order; one whose
//! result is the flags answers what `cmp result, 0` compares.

use std::collections::BTreeMap;

use iced_x86::Register;
use llrm_mir::{Attribute, ConstantId, GlobalId, Linkage, Module, Type, TypeId};
use llrm_qbruntime::{self as runtime, Contract, Control, Memory};
use llrm_x86_bcmachine::model::ir::nodes::{Node, span};
use llrm_x86_bcmachine::support::hash::IndexMap;

use crate::RUNTIME;
pub use crate::machine::{Answer, Interface};
use crate::machine::{FLAGS, FRAME_ENTRY, FRAME_EXIT, Facts, Registers, TRACKED, Word, Words, from_contract};
use crate::sites;

/// A callee, declared.
#[derive(Clone, Debug)]
pub struct Callee {
    pub reference: ConstantId,
    pub global: GlobalId,
    pub ty: TypeId,
    pub convention: u32,
    /// Register inputs, tracked roots, in slot order.
    pub inputs: Vec<Register>,
    /// Stack argument bytes.
    pub stack: i64,
    /// Whether it pops them.
    pub pops: bool,
    pub answer: Answer,
    pub never_returns: bool,
}

/// Each runtime routine the module calls, by BC's name, or why it cannot be.
#[derive(Clone, Debug, Default)]
pub struct Callees {
    pub named: BTreeMap<String, Result<Callee, String>>,
}

/// Each procedure's interface, by name.
pub fn interfaces(facts: &Facts) -> BTreeMap<String, Result<Interface, String>> {
    facts.bodies.iter().filter_map(|body| Some((body.body.name.clone()?, body.interface.clone()?))).collect()
}

/// The type an answer is.
pub fn answer_type(
    module: &mut Module,
    answer: &Answer,
) -> TypeId {
    let types = &mut module.context.types;
    match answer {
        Answer::None => types.void(),
        Answer::Flags => types.int(16),
        Answer::Registers(registers) => match registers[..] {
            [_] => types.int(16),
            [Register::EAX, Register::EDX] => types.int(32),
            _ => {
                let word = types.int(16);
                types.intern(Type::Struct { fields: vec![word; registers.len()], packed: false })
            }
        },
    }
}

/// The memory attribute a contract's reads and writes promise: a routine
/// that touches only its own pushed arguments touches no memory MIR names,
/// since MIR passes them by value.
/// One that writes a named cell (`cells`) writes memory whatever its
/// contract says.
fn memory(
    contract: &Contract,
    writes_named: bool,
) -> Option<Attribute> {
    let quiet = |one: Memory| one <= Memory::Arguments;
    let effect = match (quiet(contract.reads), quiet(contract.writes) && !writes_named) {
        (true, true) => "none",
        (false, true) => "read",
        (true, false) => "write",
        (false, false) => return None,
    };
    Some(Attribute::Memory(vec![(None, effect.to_owned())]))
}

/// Declares every runtime routine the module's bodies call.
pub fn declare(
    facts: &Facts,
    module: &mut Module,
    procedures: &BTreeMap<String, Result<Interface, String>>,
) -> Callees {
    let mut sites_of: IndexMap<String, Vec<(usize, &Contract, Words)>> = IndexMap::default();
    let mut dispatches = false;
    for body in &facts.bodies {
        let live = body.live_after(&facts.contracts);
        for node in body.nodes.values() {
            // The event-poll adapter's work is B$EVCK's.
            let (name, at) = match &**node {
                Node::Call(call) => (call.name.as_str(), call.insn.at),
                other if facts.event_poll(other) => (EVENT_POLL, span(other).0),
                _ => continue,
            };
            if llrm_qbruntime::INLINE_TABLE.contains(name) {
                dispatches = true;
                continue;
            }
            if procedures.contains_key(name)
                || [FRAME_ENTRY, FRAME_EXIT].contains(&name)
                || crate::emit::owns(name)
                || sites::meaning(name).is_some()
                || crate::floats::absorbed(name)
                || name == crate::access::ADDRESS
            {
                continue;
            }
            let Some(contract) = facts.contract(at) else { continue };
            let after = live.get(&(at as i64)).cloned().unwrap_or_default();
            sites_of.entry(name.to_owned()).or_default().push((at, contract, after));
        }
    }
    let mut callees = Callees::default();
    let family = facts.family();
    for (name, sites) in sites_of {
        let made = declared(module, facts.spaces.far, &name, &sites, family.value(), facts.handlers);
        callees.named.insert(name, made);
    }
    // A dispatch out of its table's range is B$SERR's error.
    if dispatches {
        let contract = runtime::per_call(
            &IndexMap::from_iter([(0, ERROR.to_owned())]),
            facts.family().value(),
            &Default::default(),
        )
        .swap_remove(&0)
        .expect("one contract");
        let made =
            declared(module, facts.spaces.far, ERROR, &[(0, &contract, Words::new())], family.value(), facts.handlers);
        callees.named.insert(ERROR.to_owned(), made);
    }
    callees
}

/// What the event-poll adapter calls, as QB 4.5 calls it directly.
pub const EVENT_POLL: &str = "B$EVCK";
/// The ERROR statement: raises the error its argument numbers.
pub const ERROR: &str = "B$SERR";
/// Illegal function call's error number.
pub const ILLEGAL_FUNCTION_CALL: i128 = 5;

fn declared(
    module: &mut Module,
    far: u32,
    name: &str,
    sites: &[(usize, &Contract, Words)],
    family: &str,
    handlers: bool,
) -> Result<Callee, String> {
    let contract = sites[0].1;
    if !contract.established {
        return Err(format!("{name}'s contract is not established"));
    }
    if !runtime::established_inputs(contract) {
        return Err(format!("{name}'s inputs are not established"));
    }
    if contract.error_handling {
        return Err(format!("{name} is ON ERROR machinery"));
    }
    // A routine that may enter user code comes back once that code returns;
    // with no handler of this module to enter, only other modules' code runs.
    let returns = contract.control == Control::Unknown && contract.enters_user_code && !handlers;
    if !matches!(contract.control, Control::Returns | Control::Never) && !returns {
        return Err(format!("{name}'s control is {}", contract.control.value()));
    }
    // A routine `arrays` sizes per site pops what that site pushed.
    let sized = crate::arrays::sized(name);
    let (stack, pops) = match contract.cleanup.filter(|&one| one >= 0) {
        _ if sized => (0, true),
        Some(cleanup) if cleanup > 0 => (cleanup, true),
        Some(_) => (contract.caller_cleanup, false),
        None => return Err(format!("{name}'s stack cleanup is unknown")),
    };
    if stack % 2 != 0 {
        return Err(format!("{name} takes an odd number of stack bytes"));
    }
    let mut inputs: Registers = Registers::new();
    let mut results: Registers = Registers::new();
    let mut flags = false;
    for (_, site, after) in sites {
        if !sized && (site.cleanup != contract.cleanup || site.caller_cleanup != contract.caller_cleanup) {
            return Err(format!("{name}'s calls pop different byte counts"));
        }
        inputs.extend(runtime::direct_slots(site).into_iter().filter_map(from_contract).filter(|&one| one != FLAGS));
        let disturbed: Registers = runtime::disturbs(site).into_iter().filter_map(from_contract).collect();
        for word in after {
            match *word {
                Word::Low(root) if disturbed.contains(&root) => {
                    results.insert(root);
                }
                Word::High(root) if disturbed.contains(&root) => {
                    return Err(format!("reads the high word of {root:?} after {name}"));
                }
                _ => {}
            }
        }
        flags |= after.contains(&Word::Flags) && site.flags_result;
    }
    let order = |set: &Registers| -> Vec<Register> { TRACKED.into_iter().filter(|one| set.contains(one)).collect() };
    let answer = match (flags, results.is_empty()) {
        (true, true) => Answer::Flags,
        (true, false) => return Err(format!("{name} answers in the flags and in registers")),
        (false, true) => Answer::None,
        (false, false) => Answer::Registers(order(&results)),
    };
    let inputs = order(&inputs);
    let word = module.context.types.int(16);
    let parameters = vec![word; inputs.len() + (stack / 2) as usize];
    let returns = answer_type(module, &answer);
    let ty = module.context.types.intern(Type::Function { returns, parameters, variadic: sized });
    let global = module.add_function(&format!("{RUNTIME}{name}"), ty, Linkage::External)?;
    let convention = if pops { llrm_mir::opcode::BASIC } else { 0 };
    let never_returns = contract.control == Control::Never;
    {
        let one = &mut module.globals[global.0 as usize];
        one.address_space = far;
        let llrm_mir::GlobalKind::Function(function) = &mut one.kind else { unreachable!("a function") };
        function.calling_convention = convention;
        function
            .attrs
            .extend(memory(contract, runtime::named_writes(name, family).is_some_and(|cells| !cells.is_empty())));
        if !contract.raises_error {
            function.attrs.push(llrm_mir::facts::Fact::NoUnwind.carrier());
        }
        if never_returns {
            function.attrs.push(llrm_mir::facts::Fact::NoReturn.carrier());
        }
    }
    let reference = module.reference(global);
    Ok(Callee { reference, global, ty, convention, inputs, stack, pops, answer, never_returns })
}
