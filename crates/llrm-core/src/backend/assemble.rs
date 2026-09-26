//! A MIR module to masm, as LLVM's `llc` takes a module to an object:
//! each defined function selected and run through the machine phases, each
//! defined variable's data, and the symbols both name.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::{GlobalId, GlobalKind, Linkage, Module};

use crate::abi::runtime::Contract;
use crate::backend::cpu::{Profile, ProfileOrName};
use crate::backend::isel::{self, Selected};
use crate::backend::{addressvalues, frame, globals, jumps, masm};
use crate::flow;
use crate::model::lir::LirBody;
use crate::model::ir::Space;
use crate::support::hash::IndexMap;

/// What only the frontend knows: each call's contract, and the symbol each
/// MIR name links as.
pub trait Abi {
    /// The contract of a call to `callee`: whether it pops its own
    /// arguments, and how many bytes were pushed.
    fn contract(&self, callee: &str, pops: bool, pushed: i64) -> Result<Contract, String>;
    fn linked(&self, name: &str) -> String;
}

/// `module` as masm, its code in the segment `code`.
pub fn assembled(module: &Module, abi: &dyn Abi, code: &str, cpu: ProfileOrName<'_>) -> Result<masm::Module, String> {
    let cpu = crate::backend::cpu::profile(cpu)?;
    let names = globals::names(module, &|name| abi.linked(name))?;
    let contracts = |callee: &str, pops: bool, pushed: i64| abi.contract(callee, pops, pushed);
    let mut procedures = Vec::new();
    let mut referenced: IndexMap<String, bool> = IndexMap::default();
    let mut data = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let id = GlobalId(at as u32);
        let name = global.name.as_deref().unwrap_or_default();
        match &global.kind {
            GlobalKind::Variable(variable) if variable.initializer.is_some() => data.extend(globals::datums(module, id, &names)?),
            GlobalKind::Function(function) if !function.is_declaration() => {
                let unselected = |error: isel::Unselected| format!("@{name}: {}", error.0);
                let Selected { body, convention, calls, far } = isel::selected(module, name, &contracts).map_err(unselected)?;
                let (body, reserve) = machine(body, &calls, cpu, convention.popped)?;
                let mut callees = IndexMap::default();
                for (at, callee) in &calls {
                    let linked = abi.linked(callee);
                    referenced.insert(linked.clone(), far.contains(at));
                    callees.insert(*at, masm::Callee::new(linked, far.contains(at)));
                }
                let procedure = masm::Procedure {
                    name: names[&(Space::Segment, at as i64)].clone(),
                    public: global.linkage == Linkage::External,
                    far: isel::far(global).map_err(unselected)?,
                    body,
                    reserve,
                    callees,
                    interrupt: None,
                };
                let overhead = masm::return_overhead_bytes(&procedure).map_err(|error| error.to_string())? as i64;
                let body = jumps::duplicated_returns(procedure.body.clone(), overhead);
                procedures.push(masm::Procedure { body, ..procedure });
            }
            _ => {}
        }
    }
    let defined: BTreeSet<&str> = procedures.iter().map(|one| one.name.as_str()).collect();
    let mut externs: Vec<(String, String)> = referenced
        .iter()
        .filter(|(name, _)| !defined.contains(name.as_str()))
        .map(|(name, &far)| (name.clone(), if far { "far" } else { "near" }.to_owned()))
        .collect();
    externs.sort();
    Ok(masm::Module {
        code: code.to_owned(),
        names,
        externs,
        publics: procedures.iter().filter(|one| one.public).map(|one| one.name.clone()).collect(),
        data: vec![("_DATA".to_owned(), data)],
        procedures,
        private: BTreeSet::new(),
        requests: BTreeSet::new(),
    })
}

/// A selected body through the machine phases, returning `popped` bytes;
/// and the bytes its frame reserves below BP.
fn machine(body: LirBody, calls: &IndexMap<i64, String>, cpu: &Profile, popped: i64) -> Result<(LirBody, i64), String> {
    let mut body = flow::verified(body, "isel", true).map_err(|error| error.0)?;
    let frame = Rc::new(RefCell::new(frame::of(&body, Some(calls), "", None).map_err(|error| error.0)?));
    let pinned = body.pins.clone();
    let mut in_ssa = true;
    for mut phase in flow::machine(&pinned, Some(Rc::clone(&frame)), Some(calls), false, ProfileOrName::Profile(cpu))? {
        // masm writes the prologue from the frame's reserve.
        if phase.class_name() == "Prologue" {
            continue;
        }
        if phase.class_name() == "PhiElimination" {
            in_ssa = false;
        }
        body = flow::checked(body, phase.as_mut(), in_ssa).map_err(|error| match error {
            flow::Checked::Refused(raised) => raised.message,
            flow::Checked::Malformed(malformed) => malformed.0,
        })?;
        if std::env::var_os("ISEL_DUMP").is_some() {
            println!("{}", crate::tools::stages::lir_stage(phase.class_name(), &[(body.name.clone(), body.clone())]));
        }
    }
    let body = masm::cleaned_returns(&addressvalues::converted(&body), popped)?;
    let frame = frame.borrow();
    Ok((body, -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor)))
}
