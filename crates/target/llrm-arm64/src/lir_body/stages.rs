//! Each shared machine phase run on the producer's body, one function at a
//! time (`LLRM_ARM64_STAGES=1`): what the phase did, or the first x86 fact it
//! met. The body is the selector's, phis and all, so the phases before
//! `PhiElimination` read SSA. A refused phase is skipped and the next one gets
//! the last good body, so every phase reports once per function.

use std::cell::RefCell;
use std::rc::Rc;

use llrm_codegen::backend::classes::RegisterClasses;
use llrm_codegen::backend::lanes::enter;
use llrm_codegen::backend::{cpu::tuned_for, peep::Rules, shadow::Selector, target::Segments};
use llrm_codegen::flow::{self, Checked};
use llrm_codegen::model::lir::LirBody;
use llrm_lir::registers::Regs;
use llrm_target::Target;

use crate::description::Arm64;

/// A phase's outcome on one function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stage {
    pub name: String,
    pub outcome: Outcome,
    /// The instructions the body has after the phase.
    pub insns: usize,
    /// Whether the phase changed any instruction: "ran" on a body it had
    /// nothing to do with is no evidence the phase works for the target.
    pub changed: bool,
}

fn shape(body: &LirBody) -> Vec<String> {
    body.iter_insns().map(|insn| format!("{:?}", insn.what)).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// Ran and returned a well-formed body.
    Ran,
    /// Ran; the verifier's complaints are all the producer's own (the first
    /// is shown), not the phase's.
    Inherited(String),
    /// Not run: its input is not SSA.
    NotRun,
    /// The phase refused (`module::kind: message`).
    Refused(String),
    /// The phase returned a body the verifier rejects.
    Malformed(String),
    /// The phase panicked (`file:line: message`).
    Panicked(String),
}

thread_local! {
    static WHERE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Run every kept phase over `body`; the last good body is returned with each
/// phase's outcome.
pub fn run(body: LirBody) -> Result<(LirBody, Vec<Stage>), String> {
    let arch = Arm64;
    let profile = tuned_for(&arch, "generic", false)?;
    let regs = llrm_lir::registers::Regs(arch.registers());
    let segments = Segments::of(&arch.machine(), regs, arch.operand_bytes());
    let classes = Rc::new(RegisterClasses::of(&arch));
    let omitted: Vec<&str> = arch.omitted_phases().iter().map(|(name, _)| *name).collect();
    let phases = flow::machine_with(
        &Default::default(),
        None,
        None,
        None,
        false,
        profile,
        &segments,
        &classes,
        &Rules::NONE,
        &arch.frame_registers(),
        Selector::Old,
        &omitted,
    )?;
    let roles = &arch.layout().spaces.roles;
    // The lane table is per thread: without it the phases read x86's.
    let _file = enter(Regs(&crate::REGISTER_INFO));
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        let at = info.location().map(|at| format!("{}:{}", at.file(), at.line())).unwrap_or_default();
        WHERE.with(|cell| *cell.borrow_mut() = at);
    }));
    // The phases up to PhiElimination read SSA and it makes the first body that
    // is not.
    let ssa_phases = phases.iter().position(|phase| phase.class_name() == "PhiElimination").unwrap_or(0);
    let mut body = body;
    let produced = complaints(&body, true, roles);
    let function_name = body.name.clone();
    let mut shown = shape(&body);
    let mut stages = vec![Stage {
        name: "Produced".into(),
        outcome: produced
            .first()
            .map_or(Outcome::Ran, |first| Outcome::Malformed(format!("{} complaints, first: {first}", produced.len()))),
        insns: shown.len(),
        changed: false,
    }];
    for (at, mut phase) in phases.into_iter().enumerate() {
        let name = phase.class_name().to_owned();
        let input = body.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            flow::checked(input, phase.as_mut(), at <= ssa_phases, None, &classes, &segments, &arch)
        }));
        let outcome = match result {
            Ok(Ok(out)) => {
                let now = complaints(&out, at < ssa_phases, roles);
                let new = now.iter().find(|one| !produced.contains(one));
                let outcome = match (new, now.first()) {
                    (Some(new), _) => Outcome::Malformed(new.clone()),
                    (None, Some(first)) => Outcome::Inherited(first.clone()),
                    (None, None) => Outcome::Ran,
                };
                body = out;
                outcome
            }
            Ok(Err(Checked::Refused(why))) => {
                Outcome::Refused(format!("{}::{}: {}", why.module, why.kind, why.message))
            }
            Ok(Err(Checked::Malformed(why))) => Outcome::Malformed(why.0),
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                    .unwrap_or_default();
                Outcome::Panicked(format!("{}: {message}", WHERE.with(|cell| cell.borrow().clone())))
            }
        };
        let now = shape(&body);
        if std::env::var("LLRM_ARM64_STAGES_DUMP").is_ok_and(|want| want == name) {
            eprintln!("=== {name} @{} before\n{}\n=== after\n{}", function_name, shown.join("\n"), now.join("\n"));
        }
        stages.push(Stage { name, outcome, insns: now.len(), changed: now != shown });
        shown = now;
    }
    std::panic::set_hook(previous);
    Ok((body, stages))
}

/// One tab-separated line per phase, for a corpus tally.
pub fn report(
    function: &str,
    stages: &[Stage],
) -> String {
    let mut out = String::new();
    for (at, stage) in stages.iter().enumerate() {
        let (word, why) = match &stage.outcome {
            Outcome::Ran => ("ran", ""),
            Outcome::Inherited(why) => ("inherited", why.as_str()),
            Outcome::NotRun => ("notrun", "input is not SSA"),
            Outcome::Refused(why) => ("refused", why.as_str()),
            Outcome::Malformed(why) => ("malformed", why.as_str()),
            Outcome::Panicked(why) => ("panicked", why.as_str()),
        };
        out.push_str(&format!(
            "STAGE\t{at}\t{}\t{function}\t{word}\t{}\t{}\t{}\n",
            stage.name,
            stage.insns,
            u8::from(stage.changed),
            why.replace('\n', " ")
        ));
    }
    out
}

/// Run the phases on `function`'s producer body and print the report.
pub fn observe(function: &crate::mfunc::MFunc) -> Result<(), String> {
    let lowered = super::to_lir(function).map_err(|super::Unmapped(form)| format!("unmapped form {form}"))?;
    let (_, stages) = run(lowered.body)?;
    eprint!("{}", report(&function.name, &stages));
    let (copy_webs, mixed) = multiple_definitions(function);
    eprintln!("DEFS\t{}\t{copy_webs}\t{mixed}", function.name);
    Ok(())
}

/// The phases of the pin's SSA optimization group that move or merge code:
/// `MachineCSE`, then `MachineSink` (`addMachineSSAOptimization` order). The
/// sink alone leaves the per-use copies of a constant the selector made in the
/// entry, and sinks each into its use block; the CSE first makes them one
/// value with several users, which has no block to sink into.
const SSA_GROUP: [&str; 2] = ["MachineCSE", "MachineSink"];

/// `function` after the shared phases of [`SSA_GROUP`], which run on the
/// selector's body with its phis (the pin runs them before `PhiElimination`),
/// fed the edge odds in `function.alloc_odds`. A phase that refuses leaves an
/// error, not a silently unsunk body.
pub fn sink(function: &crate::mfunc::MFunc) -> Result<crate::mfunc::MFunc, String> {
    let lowered = super::to_lir(function).map_err(|super::Unmapped(form)| format!("unmapped form {form}"))?;
    let arch = Arm64;
    let profile = tuned_for(&arch, "generic", false)?;
    let regs = Regs(arch.registers());
    let segments = Segments::of(&arch.machine(), regs, arch.operand_bytes());
    let classes = Rc::new(RegisterClasses::of(&arch));
    let omitted: Vec<&str> = arch.omitted_phases().iter().map(|(name, _)| *name).collect();
    let phases = flow::machine_with(
        &Default::default(),
        None,
        None,
        None,
        false,
        profile,
        &segments,
        &classes,
        &Rules::NONE,
        &arch.frame_registers(),
        Selector::Old,
        &omitted,
    )?;
    let mut chosen: Vec<_> = phases.into_iter().filter(|phase| SSA_GROUP.contains(&phase.class_name())).collect();
    if chosen.len() != SSA_GROUP.len() {
        return Err(format!("the shared phase list has {} of {SSA_GROUP:?}", chosen.len()));
    }
    // The lane table is per thread: without it the phases read x86's.
    let _file = enter(Regs(&crate::REGISTER_INFO));
    let mut body = lowered.body.clone();
    for phase in &mut chosen {
        let name = phase.class_name();
        body = flow::checked(body, phase.as_mut(), true, None, &classes, &segments, &arch)
            .map_err(|why| format!("{name}: {why:?}"))?;
    }
    let after = super::Lowered { body, debug_values: lowered.debug_values, tied: lowered.tied };
    super::from_lir(&after, function).map_err(|super::Unmapped(form)| format!("no way back for {form}"))
}

/// The verifier's complaints about `body`, each without its position (an
/// instruction's place moves between phases; the complaint stays).
fn complaints(
    body: &LirBody,
    in_ssa: bool,
    roles: &llrm_mir::spaces::Spaces,
) -> Vec<String> {
    llrm_codegen::backend::verify::verify_for(body, in_ssa, roles)
        .into_iter()
        .map(|one| match one.split_once(' ') {
            Some((place, rest)) if place.starts_with("0x") => rest.to_owned(),
            _ => one,
        })
        .collect()
}

/// The virtual registers defined more than once: (all by copies, which is a
/// phi's lowering; some by another operation, which is not SSA either way).
fn multiple_definitions(function: &crate::mfunc::MFunc) -> (usize, usize) {
    use std::collections::HashMap;
    let mut defs: HashMap<crate::mc::visit::Reg, (usize, bool)> = HashMap::new();
    for op in function.blocks.iter().flat_map(|block| block.ops.iter()) {
        let copy = matches!(op, crate::mfunc::Op::Copy { .. });
        for register in op.defs_and_uses().0 {
            let seen = defs.entry(register).or_insert((0, true));
            seen.0 += 1;
            seen.1 &= copy;
        }
    }
    let virtual_ = |register: &crate::mc::visit::Reg| match register {
        crate::mc::visit::Reg::Gpr(crate::mc::inst::Gpr(n)) => *n >= crate::mc::inst::Gpr::FIRST_VIRTUAL,
        crate::mc::visit::Reg::Fpr(crate::mc::inst::Vreg(n)) => *n >= crate::mc::inst::Gpr::FIRST_VIRTUAL,
    };
    let many = defs.iter().filter(|(register, (count, _))| *count > 1 && virtual_(register));
    let copies = many.clone().filter(|(_, (_, only))| *only).count();
    if std::env::var("LLRM_ARM64_STAGES_MIXED").is_ok() {
        for (register, _) in many.clone().filter(|(_, (_, only))| !*only) {
            for op in function.blocks.iter().flat_map(|block| block.ops.iter()) {
                if op.defs_and_uses().0.contains(register) {
                    eprintln!("MIXED {} {register:?}: {op:?}", function.name);
                }
            }
        }
    }
    (copies, many.count() - copies)
}
