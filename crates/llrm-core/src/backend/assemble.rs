//! A MIR module to masm, as LLVM's `llc` takes a module to an object:
//! each defined function selected and run through the machine phases, each
//! defined variable's data, and the symbols both name.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::{GlobalId, GlobalKind, Linkage, Module};

use crate::abi::runtime::Contract;
use crate::backend::cpu::{Profile, ProfileOrName};
use crate::backend::target::Segments;
use crate::backend::constpool::Pool;
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
    /// What a call to `callee` passes and answers in registers rather than
    /// on the stack, where its ABI names them.
    fn registers(&self, _callee: &str) -> Option<Registers> {
        None
    }
}

/// A routine's register interface: its last `arguments.len()` arguments,
/// each in its register, and its result's fields, each in its register.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    pub arguments: Vec<iced_x86::Register>,
    pub results: Vec<iced_x86::Register>,
}

/// `module` as masm, its code in the segment `code`.
pub fn assembled(module: &Module, abi: &dyn Abi, code: &str, cpu: ProfileOrName<'_>, segments: &Segments) -> Result<masm::Module, String> {
    let cpu = crate::backend::cpu::profile(cpu)?;
    let mut names = globals::names(module, &|name| abi.linked(name))?;
    names.extend(crate::hir::lower::symbol_names());
    let mut procedures = Vec::new();
    let mut referenced: IndexMap<String, bool> = IndexMap::default();
    let mut data = Vec::new();
    let pool = Rc::new(RefCell::new(Pool::new(module.globals.len() as i64)));
    let target = Target { cpu, segments, runtime: "", basic: false, zeroed: false };
    for (at, global) in module.globals.iter().enumerate() {
        let id = GlobalId(at as u32);
        let name = global.name.as_deref().unwrap_or_default();
        match &global.kind {
            GlobalKind::Variable(variable) if variable.initializer.is_some() => data.extend(globals::datums(module, id, &names)?),
            GlobalKind::Function(function) if !function.is_declaration() => {
                let unselected = |error: isel::Unselected| format!("@{name}: {}", error.0);
                let Machined { body, reserve, calls, inline, far, popped, .. } = machined(module, name, abi, &pool, &target)?;
                let body = masm::cleaned_returns(&addressvalues::converted(&body), popped)?;
                let mut callees = IndexMap::default();
                for (at, callee) in &calls {
                    if let Some(code) = inline.get(at) {
                        callees.insert(*at, masm::Callee { name: callee.clone(), far: false, code: vec![masm::InlinePart::Bytes(code.clone())] });
                        continue;
                    }
                    // A global of this module is called by the name it is defined or declared as.
                    let linked = match module.named(callee) {
                        Some(id) => names[&(globals::space(module, id), i64::from(id.0))].clone(),
                        None => abi.linked(callee),
                    };
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
    for (bytes, id) in pool.borrow().entries() {
        let name = format!("$K{id}");
        names.insert((Space::Segment, id), name.clone());
        data.extend([masm::Datum::Label(masm::Label { name }), masm::Datum::Bytes(bytes.to_vec())]);
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

/// What the machine a frontend compiles for is: its processor, its segment
/// registers, the runtime family whose frame it calls into, and whether
/// floats keep BASIC's semantics.
pub struct Target<'t> {
    pub cpu: &'t Profile,
    pub segments: &'t Segments,
    pub runtime: &'t str,
    pub basic: bool,
    /// A framed function's locals start zeroed: B$ENRA zero-fills them.
    pub zeroed: bool,
}

/// A function selected and through the machine phases, as llc's
/// per-function pipeline: its LIR, returns not yet cleaned, the bytes its
/// frame reserves below BP, each call's callee by the call's `at` and
/// which are far, and the bytes it pops.
pub struct Machined {
    pub body: LirBody,
    pub reserve: i64,
    pub calls: IndexMap<i64, String>,
    /// The code laid down in place of each call to an inline helper.
    pub inline: IndexMap<i64, Vec<u8>>,
    pub far: BTreeSet<i64>,
    pub popped: i64,
    /// The landing pad's block, laid out last, which the statement table
    /// gives the runtime.
    pub landing: Option<i64>,
}

/// `name` of `module` selected and run through the machine phases, float
/// constants in `pool`.
pub fn machined(module: &Module, name: &str, abi: &dyn Abi, pool: &Rc<RefCell<Pool>>, target: &Target<'_>) -> Result<Machined, String> {
    let zeroed = target.zeroed && module.named(name).is_some_and(|global| crate::driver::framed(module, global));
    let selected = isel::selected(module, name, abi, &mut pool.borrow_mut(), target.cpu, target.segments, zeroed);
    let Selected { body, convention, calls, inline, far, depth, landing } = selected.map_err(|error| format!("@{name}: {}", error.0))?;
    let mut body = flow::verified(body, "isel", true).map_err(|error| error.0)?;
    let mut frame = frame::of(&body, Some(&calls), target.runtime, None).map_err(|error| error.0)?;
    frame.floor = frame.floor.min(-depth);
    let frame = Rc::new(RefCell::new(frame));
    let pinned = body.pins.clone();
    let mut in_ssa = true;
    for mut phase in flow::machine(&pinned, Some(Rc::clone(&frame)), Some(Rc::clone(pool)), Some(&calls), target.basic, ProfileOrName::Profile(target.cpu), target.segments)? {
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
    let frame = frame.borrow();
    let reserve = -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor);
    let (body, landing) = match landing {
        Some(marker) => {
            let (body, at) = landed_last(body, marker).map_err(|error| format!("@{name}: {error}"))?;
            (body, Some(at))
        }
        None => (body, None),
    };
    Ok(Machined { body, reserve, calls, inline, far, popped: convention.popped, landing })
}

/// `body` with its landing pad, the block `marker` starts, laid out last:
/// the runtime resumes at the first statement-table row at or after the
/// faulting call, and the pad is the function's one row.
fn landed_last(body: LirBody, marker: i64) -> Result<(LirBody, i64), String> {
    let starts = |block: &crate::model::lir::LirBlock| block.insns.iter().find(|one| masm::prints(one)).is_some_and(|one| one.at == marker);
    let Some(index) = body.blocks.iter().position(starts) else {
        return Err("the machine phases moved the landing pad's start".to_owned());
    };
    let mut blocks = body.blocks.clone();
    let pad = blocks.remove(index);
    let at = pad.at;
    blocks.push(pad);
    Ok((body.with_blocks(blocks), at))
}
