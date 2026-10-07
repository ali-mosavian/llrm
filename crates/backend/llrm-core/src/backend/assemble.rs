//! A MIR module to masm, as LLVM's `llc` takes a module to an object:
//! each defined function selected and run through the machine phases, each
//! defined variable's data, and the symbols both name.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::facts::Fact;
use llrm_mir::{GlobalId, GlobalKind, Linkage, Module};

use crate::abi::runtime::Contract;
use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::{Profile, ProfileOrName};
use crate::backend::target::Segments;
use crate::backend::constpool::Pool;
use crate::backend::isel::{self, Selected};
use crate::backend::{addressvalues, executed, frame, globals, jumps, masm, select, ssaspill};
use crate::flow;
use llrm_support::debug::timed;
use crate::model::lir::LirBody;
use crate::model::ir::{Addr, Space};
use crate::support::hash::IndexMap;

/// What only the frontend knows: each call's contract, and the symbol each
/// MIR name links as.
pub trait Abi {
    /// The contract of a call to `callee`: whether it pops its own
    /// arguments, and how many bytes were pushed.
    fn contract(&self, callee: &str, pops: bool, pushed: i64) -> Result<Contract, String>;
    fn linked(&self, name: &str) -> String;
    /// Where the runtime keeps its stack's lower limit and what overflowing it calls, where the
    /// program checks its stack.
    fn stack_check(&self) -> Option<&masm::StackCheck> {
        None
    }
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

/// `module` as masm, its code in the segment `code`, selected by the 16-bit x86
/// selector: what the tests of this crate are written for.
#[cfg(test)]
pub fn assembled(module: &Module, abi: &dyn Abi, code: &str, cpu: ProfileOrName<'_>, segments: &Segments) -> Result<masm::Module, String> {
    assembled_by(module, abi, code, cpu, segments, isel::m16(), &llrm_x86_m16::M16)
}

/// `module` as masm, its code in the segment `code`, selected by `selection`.
pub fn assembled_by(module: &Module, abi: &dyn Abi, code: &str, cpu: ProfileOrName<'_>, segments: &Segments, selection: &'static isel::Compiled, arch: &dyn llrm_target::Target) -> Result<masm::Module, String> {
    let cpu = crate::backend::cpu::profile(cpu)?;
    let module = &*timed("mir near code", || crate::backend::nearcode::placed(module));
    let mut names = timed("global names", || globals::names(module, &|name| abi.linked(name)))?;
    names.extend(crate::hir::symbols::symbol_names());
    let mut procedures = Vec::new();
    let mut referenced: IndexMap<String, bool> = IndexMap::default();
    let mut data = Vec::new();
    let pool = Rc::new(RefCell::new(Pool::new(module.globals.len() as i64)));
    let classes = Rc::new(RegisterClasses::of(arch));
    let target = Target { cpu, segments, selection, arch, classes: &classes, runtime: "", basic: false, zeroed: false };
    for (at, global) in module.globals.iter().enumerate() {
        let id = GlobalId(at as u32);
        let name = global.name.as_deref().unwrap_or_default();
        match &global.kind {
            GlobalKind::Variable(variable) if variable.initializer.is_some() => data.extend(globals::datums(module, id, &names)?),
            GlobalKind::Function(function) if !function.is_declaration() => {
                let unselected = |error: isel::Unselected| format!("@{name}: {}", error.0);
                let Machined { body, reserve, calls, inline, far, pops, popped, registers, .. } = llrm_support::debug::in_function(name, || machined(module, name, abi, &pool, &target))?;
                let body = timed("masm cleaned returns", || masm::cleaned_returns(&addressvalues::converted(&body), popped))?;
                let mut callees = IndexMap::default();
                for (at, callee) in &calls {
                    if let Some(code) = inline.get(at) {
                        callees.insert(*at, masm::Callee { name: callee.clone(), far: false, pops: 0, code: vec![masm::InlinePart::Bytes(code.clone())] });
                        continue;
                    }
                    // A global of this module is called by the name it is defined or declared as.
                    let linked = match module.named(callee) {
                        Some(id) => names[&(globals::space(module, id), i64::from(id.0))].clone(),
                        None => abi.linked(callee),
                    };
                    referenced.insert(linked.clone(), far.contains(at));
                    callees.insert(*at, masm::Callee { pops: pops.get(at).copied().unwrap_or(0), ..masm::Callee::new(linked, far.contains(at)) });
                }
                // An interrupt handler loads DGROUP into DS itself.
                let interrupt = (function.calling_convention == llrm_mir::opcode::X86_INTR).then(|| {
                    let (space, index) = crate::hir::symbols::DGROUP;
                    names.insert((space, index), globals::DGROUP.to_owned());
                    Addr { index, ..Addr::new(space, 0) }
                });
                let procedure = masm::Procedure {
                    name: names[&(Space::Segment, at as i64)].clone(),
                    public: global.linkage == Linkage::External,
                    far: isel::far(global).map_err(unselected)?,
                    body,
                    reserve,
                    callees,
                    interrupt,
                    size: cpu.size,
                    entry: 0,
                    stack_check: function.attrs.iter().any(|one| Fact::of_attribute(one) == Some(Fact::StackCheck)).then(|| abi.stack_check().cloned()).flatten(),
                    registers,
                };
                let overhead = timed("masm return overhead", || masm::return_overhead_bytes(&procedure)).map_err(|error| error.to_string())? as i64;
                let body = timed("lir duplicated returns", || jumps::duplicated_returns(procedure.body.clone(), overhead));
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
    timed("stack checks", || crate::backend::stackusage::elide_checks(&mut procedures, &masm::entered_directly(module, &names), &*arch));
    // A function this module declares and names only in data (a table of function pointers) is an external too: no call says so.
    let declared: BTreeSet<&str> = module
        .globals
        .iter()
        .enumerate()
        .filter(|(_, global)| matches!(&global.kind, GlobalKind::Function(function) if function.is_declaration()))
        .filter_map(|(at, _)| names.get(&(globals::space(module, GlobalId(at as u32)), at as i64)).map(String::as_str))
        .collect();
    for datum in &data {
        if let masm::Datum::Pointer(pointer) = datum
            && declared.contains(pointer.name.as_str())
        {
            referenced.entry(pointer.name.clone()).or_insert(pointer.far);
        }
    }
    let defined: BTreeSet<&str> = procedures.iter().map(|one| one.name.as_str()).collect();
    let mut externs: Vec<(String, String)> = referenced
        .iter()
        .filter(|(name, _)| !defined.contains(name.as_str()))
        .map(|(name, &far)| (name.clone(), if far { "far" } else { "near" }.to_owned()))
        .collect();
    externs.extend(masm::stack_externs(&procedures, &mut names));
    externs.sort();
    externs.dedup();
    let debug = timed("debug info", || crate::backend::debuginfo::described(module, &names, llrm_object::debug::Producer::Native, arch))?;
    Ok(masm::Module {
        code: code.to_owned(),
        names,
        externs,
        publics: procedures.iter().filter(|one| one.public).map(|one| one.name.clone()).collect(),
        data: vec![("_DATA".to_owned(), data)],
        procedures,
        private: BTreeSet::new(),
        requests: BTreeSet::new(),
        debug,
        far_bss: BTreeSet::new(),
        stack: 0,
        object: arch.object(),
    })
}

/// What the machine a frontend compiles for is: its processor, its segment
/// registers, the runtime family whose frame it calls into, and whether
/// floats keep BASIC's semantics.
pub struct Target<'t> {
    /// What the target requires of registers: the pins of its forms.
    pub classes: &'t Rc<RegisterClasses>,
    pub cpu: &'t Profile,
    pub segments: &'t Segments,
    /// The instruction selector of the target, which its driver binds.
    pub selection: &'static isel::Compiled,
    /// The target the selection is for.
    pub arch: &'t dyn llrm_target::Target,
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
    /// The argument bytes each direct call's callee pops.
    pub pops: IndexMap<i64, i64>,
    pub popped: i64,
    /// The frame's registers, the saved ones those this function's convention keeps.
    pub registers: llrm_target::FrameRegisters,
    /// The landing pad's block, laid out last, which the statement table
    /// gives the runtime.
    pub landing: Option<i64>,
}

/// `name` of `module` selected and run through the machine phases, float
/// constants in `pool`.
///
/// Where spill slots end up past a one-byte displacement below allocas,
/// the function is selected again with the allocas below a hole the spill
/// slots fill, and whichever has fewer two-byte displacements is kept.
pub fn machined(module: &Module, name: &str, abi: &dyn Abi, pool: &Rc<RefCell<Pool>>, target: &Target<'_>) -> Result<Machined, String> {
    let kept = machined_once(module, name, abi, pool, target)?;
    // What the function's cost in `timefunc` is measured against.
    llrm_support::debug!("size", "{name} {}", kept.body.insns().len());
    // Reported once the choice is made: a rejected candidate is no function's cost.
    if crate::support::debug::enabled("cost") {
        llrm_support::debug!("cost", "{}", executed::summary(&kept.body, target.cpu));
    }
    if crate::support::debug::enabled("pressure") {
        if let Some(row) = executed::pressure(&kept.body) {
            llrm_support::debug!("pressure", "{row}");
        }
    }
    Ok(kept)
}

fn machined_once(module: &Module, name: &str, abi: &dyn Abi, pool: &Rc<RefCell<Pool>>, target: &Target<'_>) -> Result<Machined, String> {
    let (first, frame) = timed("candidate first frame", || cheaper(module, name, abi, pool, target, 0))?;
    let spilled = frame.floor + first.reserve;
    let far = far_frame(&first.body);
    if target.basic || frame.native.is_some() || spilled <= 0 || frame.floor == 0 || far == 0 {
        return Ok(first);
    }
    let (second, _) = timed("candidate second frame", || cheaper(module, name, abi, pool, target, spilled))?;
    Ok(if far_frame(&second.body) < far { second } else { first })
}

/// `phased`, with the spiller; and, where the spiller changed the body, without it too: the
/// one that costs less is kept. The spiller decides on the general registers alone, so where
/// the allocator's pressure is elsewhere (segment registers, x87 and fixed-register glue) its
/// spill code can be on top of what the allocator does anyway.
fn cheaper(module: &Module, name: &str, abi: &dyn Abi, pool: &Rc<RefCell<Pool>>, target: &Target<'_>, hole: i64) -> Result<(Machined, frame::Frame), String> {
    // `LLRM_CANDIDATES=spiller|allocator` tries one route alone, to see what each makes.
    let candidates = match std::env::var("LLRM_CANDIDATES").as_deref() {
        Ok("spiller") => Candidates::SpillerOnly,
        Ok("allocator") => Candidates::AllocatorOnly,
        _ => CANDIDATES.with(std::cell::Cell::get),
    };
    if candidates == Candidates::AllocatorOnly {
        return timed("candidate allocator alone", || phased(module, name, abi, pool, target, hole, false, true)).map(|(made, _)| made);
    }
    let (spilled, ran) = timed("candidate spiller", || phased(module, name, abi, pool, target, hole, true, true))?;
    if !ran.changed() || candidates == Candidates::SpillerOnly {
        return Ok(spilled);
    }
    let (allocator_alone, _) = timed("candidate allocator alone", || phased(module, name, abi, pool, target, hole, false, true))?;
    let (kept, from_spiller) = match timed("candidate cost", || (cost(&spilled.0, target), cost(&allocator_alone.0, target))) {
        (Some(with), Some(without)) if without < with => (allocator_alone, false),
        _ => (spilled, true),
    };
    // Where code bytes are the measure, a loop admitted because its trips are fewer, on a tie in the bytes the spiller
    // counts, is checked against the encoded code: the loads it moved to the entry are not all it changed.
    if from_spiller && ran.ties() {
        let (plain, _) = timed("candidate plain", || phased(module, name, abi, pool, target, hole, true, false))?;
        if timed("candidate cost", || cost(&plain.0, target).zip(cost(&kept.0, target))).is_some_and(|(plain, admitted)| plain < admitted) {
            return Ok(plain);
        }
    }
    Ok(kept)
}

/// Which routes through the machine phases `machined` tries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Candidates {
    /// The spiller in front of the allocator and the allocator alone, the cheaper kept.
    Both,
    SpillerOnly,
    AllocatorOnly,
}

thread_local! {
    static CANDIDATES: std::cell::Cell<Candidates> = const { std::cell::Cell::new(Candidates::Both) };
}

/// `run` with `machined` trying only `candidates` on this thread.
pub fn trying<T>(candidates: Candidates, run: impl FnOnce() -> T) -> T {
    let before = CANDIDATES.with(|one| one.replace(candidates));
    let done = run();
    CANDIDATES.with(|one| one.set(before));
    done
}

/// What a finished function costs: its encoded bytes where the target optimizes for size,
/// else the instructions and memory operands it is expected to execute per call.
fn cost(made: &Machined, target: &Target<'_>) -> Option<f64> {
    if target.cpu.size {
        made.body.insns().iter().filter_map(|one| one.what.as_ref()).map(|what| select::emit_in(made.body.bits, what, 0, None, false, false, None).map(|code| code.code.len() as f64)).sum()
    } else {
        executed::work(&made.body)
    }
}

/// The frame operands a one-byte displacement does not reach.
fn far_frame(body: &LirBody) -> usize {
    use crate::model::ir::Loc;
    body.insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .flat_map(|what| what.dests.iter().chain(&what.sources))
        .filter_map(|place| match place {
            Loc::Mem(cell) => cell.addr,
            Loc::Address(address) => address.addr,
            _ => None,
        })
        .filter(|addr| addr.space == Space::Frame && !(-128..128).contains(&addr.disp))
        .count()
}

/// `machined` with `hole` bytes left above the allocas; and the frame.
/// `machined` once through the machine phases, with the spiller or not and, if so, letting a loop's entry load what the loop reads or
/// not (`admission`); and what the spiller settled.
#[allow(clippy::too_many_arguments)]
fn phased(module: &Module, name: &str, abi: &dyn Abi, pool: &Rc<RefCell<Pool>>, target: &Target<'_>, hole: i64, spilling: bool, admission: bool) -> Result<((Machined, frame::Frame), Rc<ssaspill::Run>), String> {
    let run = ssaspill::Run::new(admission);
    let zeroed = target.zeroed && module.named(name).is_some_and(|global| crate::driver::framed(module, global));
    let selected = timed("isel", || isel::selected(module, name, abi, &mut pool.borrow_mut(), target.cpu, target.segments, target.selection, target.arch, zeroed, hole));
    let Selected { body, convention, calls, inline, far, pops, depth, landing } = selected.map_err(|error| format!("@{name}: {}", error.0))?;
    let registers = llrm_target::FrameRegisters { saved: convention.saved.clone(), ..target.arch.frame_registers() };
    let mut body = timed("lir verify", || flow::verified(body, "isel", true)).map_err(|error| error.0)?;
    let mut frame = timed("lir frame", || frame::of(&body, Some(&calls), target.runtime, None)).map_err(|error| error.0)?;
    frame.floor = frame.floor.min(-depth);
    frame.hole = hole;
    let frame = Rc::new(RefCell::new(frame));
    let pinned = body.pins.clone();
    let mut in_ssa = true;
    for mut phase in flow::machine_with(&pinned, Some(Rc::clone(&frame)), Some(Rc::clone(pool)), Some(&calls), target.basic, ProfileOrName::Profile(target.cpu), target.segments, target.classes, spilling.then(|| Rc::clone(&run)), target.selection.rules(), &registers)? {
        // masm writes the prologue from the frame's reserve.
        if phase.class_name() == "Prologue" {
            continue;
        }
        if phase.class_name() == "PhiElimination" {
            in_ssa = false;
        }
        body = flow::checked(body, phase.as_mut(), in_ssa, target.classes).map_err(|error| match error {
            flow::Checked::Refused(raised) => format!("@{name}: {}", raised.message),
            flow::Checked::Malformed(malformed) => format!("@{name}: {}", malformed.0),
        })?;
        if std::env::var_os("ISEL_DUMP").is_some() {
            println!("{}", crate::backend::lirtext::lir_stage(phase.class_name(), &[(body.name.clone(), body.clone())]));
        }
    }
    let frame = frame.borrow().clone();
    let reserve = -std::cmp::min(frame.slots.values().copied().min().unwrap_or(0), frame.floor);
    let (body, landing) = match landing {
        Some(marker) => {
            let (body, at) = timed("lir landed last", || landed_last(body, marker)).map_err(|error| format!("@{name}: {error}"))?;
            (body, Some(at))
        }
        None => (body, None),
    };
    Ok(((Machined { body, reserve, calls, inline, far, pops, popped: convention.popped, registers, landing }, frame), run))
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
