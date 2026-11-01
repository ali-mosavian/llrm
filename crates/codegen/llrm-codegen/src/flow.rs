//! The machine phases and their gate: each LIR phase run, verified and checked.
//!
//! At -O0 (`Profile::optimized` off) the phases that only improve code are not
//! run: peephole, loop slots, the scheduler, the coalescer and the pressure
//! sink. gcc's and clang's -O0 improve nothing after selection and allocation;
//! the same switch leaves out the allocator's folded reads
//! (`allocate::RegAlloc`), the copies of returns and split tails
//! (`assemble::assembled_by`) and the block placement (`jumps::ControlFlow`).

use std::cell::RefCell;
use std::rc::Rc;

use llrm_lir::registers::RegId;

use crate::backend::constpool::Pool;
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::frame::Frame;
use crate::backend::target::Segments;
use crate::backend::verify::{self, Malformed};
use crate::backend::{
    allocate, farcall, floatalloc, floatassign, jumps, loopslots, parcopy, peephole, phielim, prologue, schedule,
    twoaddr,
};
use crate::coalescer::pass::Coalescer;
use crate::model::lir::LirBody;
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

/// Every phase between instruction selection and emission, in order, with the
/// spiller in front of the allocator or left out.
#[cfg(test)]
/// The machine phases for real mode's rules, which the tests of the phases are
/// written for.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn machine<'a>(
    pinned: &IndexMap<u32, RegId>,
    frame: Option<Rc<RefCell<Frame>>>,
    pool: Option<Rc<RefCell<Pool>>>,
    calls: Option<&IndexMap<i64, String>>,
    basic_semantics: bool,
    cpu: impl Into<ProfileOrName<'a>>,
    segments: &Segments,
) -> Result<Vec<Box<dyn LIRTransform + 'a>>, String> {
    machine_with(
        pinned,
        frame,
        pool,
        calls,
        basic_semantics,
        cpu,
        segments,
        &crate::backend::classes::RegisterClasses::m16(),
        &crate::backend::targets::x86_m16::RULES,
        &llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        crate::backend::shadow::Selector::Old,
    )
}

/// The phases -O0 leaves out: each only makes code better.
const UNOPTIMIZED: &[&str] = &[
    "Peephole",
    "LoopSlots",
    "Scheduler",
    "Coalescer",
    "MachineScheduler",
    "MachineSink",
    "PeepholeOptimizer",
    "MachineCSE",
    "OptimizePHIs",
    "LateCleanup",
    "MachineCombiner",
    "PushFold",
    "LoadFold",
    "FixupSetCC",
    "CopyPropagation",
    "LiveRangeShrink",
    "X86OptimizeLEAs",
    "MachineLICM",
    "IvShare",
    "IvZero",
    "FixupBW",
    "FixupLEAs",
    "CmpAsSub",
    "CallFrameOpt",
];

/// The parity registry's slot of each machine phase, by class name.
const SLOTS: &[(&str, &str)] = &[
    ("EarlyTailDuplicate", "earlytaildup@1"),
    ("FarIndirectCalls", "farcall@1"),
    ("LoadFold", "pushfold@6"),
    ("CmpAsSub", "cmpsub@1"),
    ("OptimizePHIs", "optimizephis@1"),
    ("DeadMachineInstructionElim", "deadmi@1"),
    ("SharedConstants", "phiconst@2"),
    ("ProcessImplicitDefs", "processimpdefs@2"),
    ("PhiElimination", "phielim@4"),
    ("MachineLICM", "machinelicm@4"),
    ("IvShare", "machinelicm@4"),
    ("IvZero", "machinelicm@4"),
    ("MachineCSE", "machinecse@4"),
    ("MachineSink", "machinesink@4"),
    ("PeepholeOptimizer", "peepholeopt@4"),
    ("MachineCombiner", "combiner@4"),
    ("LiveRangeShrink", "liverangeshrink@4"),
    ("MachineScheduler", "machinesched@5"),
    ("FixupSetCC", "fixupsetcc@5"),
    ("CallFrameOpt", "callframe@5"),
    ("ReserveCallFrame", "callframe@5"),
    ("FlagsCopy", "flagscopy@5"),
    ("X86OptimizeLEAs", "leaopt@5"),
    ("FloatFold", "floatfold@6"),
    ("PushFold", "pushfold@6"),
    ("FloatAssign", "floatassign@7"),
    ("FloatPrep", "floatassign@7"),
    ("FloatAlloc", "floatalloc@8"),
    ("TwoAddress", "twoaddr@9"),
    ("Coalescer", "coalesce@10"),
    ("RegAlloc", "allocate@11"),
    ("ParallelCopy", "parcopy@12"),
    ("Prologue", "prologue@13"),
    ("LateCleanup", "latecleanup@13"),
    ("Peephole", "peephole@14"),
    ("FixupBW", "fixupbw@14"),
    ("FixupLEAs", "fixupleas@14"),
    ("LoopSlots", "loopslots@15"),
    ("Scheduler", "schedule@16"),
    ("ControlFlow", "jumps@17"),
    ("CopyPropagation", "copyprop@18"),
];

/// `machine`, its peephole made of the rules `rules` holds.
#[allow(clippy::too_many_arguments)]
pub fn machine_with<'a>(
    pinned: &IndexMap<u32, RegId>,
    frame: Option<Rc<RefCell<Frame>>>,
    pool: Option<Rc<RefCell<Pool>>>,
    calls: Option<&IndexMap<i64, String>>,
    basic_semantics: bool,
    cpu: impl Into<ProfileOrName<'a>>,
    segments: &Segments,
    classes: &Rc<crate::backend::classes::RegisterClasses>,
    rules: &'static crate::backend::peep::Rules,
    registers: &llrm_target::FrameRegisters,
    selector: crate::backend::shadow::Selector,
) -> Result<Vec<Box<dyn LIRTransform + 'a>>, String> {
    let target = targets::profile(cpu)?;
    let mut pinned = pinned.clone();
    if let Some(frame) = &frame {
        let frame = frame.borrow();
        if frame.native.is_some() {
            pinned.extend(frame.native_pins.iter().map(|(value, register)| (*value, *register)));
        }
    }
    let or_empty = || frame.clone().unwrap_or_else(|| Rc::new(RefCell::new(Frame::new(0))));
    let mut phases: Vec<Box<dyn LIRTransform + 'a>> = vec![
        // Directly after selection, as the pin has it: a small join copied into
        // the blocks that jump to it (EarlyTailDuplicate).
        Box::new(crate::backend::earlytaildup::EarlyTailDuplicate { size: target.size }),
        Box::new(farcall::FarIndirectCalls::new(or_empty())),
        // As the pin's selection has it: a once-read load is the memory operand
        // of its reader before MachineCombiner sees the chain.
        Box::new(crate::backend::pushfold::LoadFold),
        // The compare of a value with the constant it was just subtracted by
        // shares the subtract's flags (X86TargetLowering::EmitCmp).
        Box::new(crate::backend::cmpsub::CmpAsSub),
        // On SSA, first: a phi cycle that carries one value is that value (the
        // pin: OptimizePHIs, ahead of MachineCSE).
        Box::new(crate::backend::optimizephis::OptimizePhis),
        // As the pin has it, straight after selection: what selection built
        // and nothing reads (a far pointer joined for the string registers).
        Box::new(crate::backend::deadmi::DeadMachineInstructionElim),
        // Before SsaSpill: a constant several edges of a phi carry is one
        // value.
        Box::new(crate::backend::phiconst::SharedConstants),
        // Before the phis go: a value nothing defines is read as undef.
        Box::new(crate::processimpdefs::ProcessImplicitDefs { ignored_inputs: target.in_order }),
        // On SSA, ahead of the cmov conversion and EarlyMachineLICM (the pin's
        // addILPOpts): a chain of one integer operation rebuilt so its late
        // operand joins last.
        Box::new(crate::backend::combiner::MachineCombiner),
        // On SSA, a loop-invariant computation in the preheader (the pin:
        // EarlyMachineLICM, ahead of MachineCSE).
        Box::new(crate::backend::machinelicm::MachineLicm { classes: Rc::clone(classes) }),
        // After the loop-invariant values are in the preheader: streams that
        // share one index share one pointer (DEVIATE LSR2).
        Box::new(crate::backend::ivshare::IvShare { classes: Rc::clone(classes) }),
        // A counted loop's index ends at zero (DEVIATE LSR3).
        Box::new(crate::backend::ivzero::IvZero { classes: Rc::clone(classes) }),
        // On SSA, a repeated computation once (the pin: MachineCSE, then
        // MachineSink).
        Box::new(crate::backend::precse::MachineCse { classes: Rc::clone(classes) }),
        // On SSA, where a def with phi users alone can go to its edge.
        Box::new(crate::backend::machinesink::MachineSink {
            size: target.size,
            limit: classes.available.len(),
            classes: Rc::clone(classes),
        }),
        // On SSA, after sinking (the pin's PeepholeOptimizer): a subtract
        // nothing reads is a compare (optimizeCompareInstr).
        Box::new(crate::backend::peepholeopt::PeepholeOptimizer { size: target.size }),
        // On SSA, before the pin allocates: two addresses that differ in a
        // displacement are one (X86OptimizeLEAs, pre-RA).
        Box::new(crate::backend::leaopt::OptimizeLeas { size: target.size }),
        // On SSA, last: an instruction up to the last definition of what it
        // reads (the pin: LiveRangeShrink, x86 addPreRegAlloc).
        Box::new(crate::backend::liverangeshrink::LiveRangeShrink {
            segments: segments.clone(),
            classes: Rc::clone(classes),
        }),
        // On SSA, as the pin's `addPreRegAlloc` has it
        // (X86CallFrameOptimization).
        Box::new(crate::backend::callframe::CallFrameOpt {
            slot: registers.slot,
            align: registers.align,
            stack: registers.stack,
            // A function with variable-sized objects reserves no frame and
            // pushes every area (`CallFrameOpt::allocates_dynamically`).
            has_var_sized: false,
            slow_two_mem_ops: false,
        }),
        // On SSA, right after the pushes are made: a pushed value a load made,
        // read nowhere else, is pushed from the load's cell (the pin:
        // X86CallFrameOptimization's push folding).
        Box::new(crate::backend::pushfold::PushFold),
        // On SSA, as the pin's `addPreRegAlloc` has it: flags kept across an
        // instruction that clobbers them become a byte per condition
        // (X86FlagsCopyLowering, every level).
        Box::new(crate::backend::flagscopy::FlagsCopy),
        Box::new(phielim::PhiElimination),
        // After the shrinking of live ranges: a `setcc` zero-extended is a
        // zero made before the compare (the pin: X86FixupSetCC).
        Box::new(crate::backend::fixupsetcc::FixupSetCc),
        // A float load only one arithmetic instruction reads is its memory
        // operand.
        Box::new(crate::backend::floatfold::FloatFold { cpu: target }),
        // After phi elimination: a phi's copies are where the stack shuffles.
        Box::new(floatassign::FloatPrep {
            frame: frame.clone(),
            pool,
            basic_semantics,
            status_word: classes.status_word(),
        }),
        Box::new(floatalloc::FloatAlloc { frame: frame.clone(), classes: Rc::clone(classes) }),
        Box::new(twoaddr::TwoAddress {
            lea: match target.dword_address_form() {
                None if target.address_forms.iter().any(|form| form.index_width == 8) => {
                    crate::twoaddress::ThreeAddress::Wide
                }
                None => crate::twoaddress::ThreeAddress::Off,
                Some(form) if form.secondary => crate::twoaddress::ThreeAddress::AddSubWords,
                Some(_) => crate::twoaddress::ThreeAddress::All,
            },
        }),
        Box::new(Coalescer { size: target.size, ..Coalescer::new(None, segments, classes) }),
        // After TwoAddress and the coalescer, before any value is placed: the
        // pin's pre-RA scheduler, bottom-up by pressure.
        Box::new(crate::backend::machinesched::MachineScheduler {
            classes: Rc::clone(classes),
            segments: segments.clone(),
        }),
        Box::new(
            allocate::RegAlloc::new(Some(&pinned), frame.clone(), ProfileOrName::Profile(target), segments, classes)?
                .with_saved(registers.saved.iter().map(|(whole, _)| *whole).collect())
                .with_basic_semantics(basic_semantics),
        ),
        // After allocation: which moves in a phi's copy conflict is a question
        // about locations.
        Box::new(parcopy::ParallelCopy {
            prices: parcopy::CyclePrices::of(target),
            available: classes.available.clone(),
            saved: registers.saved.iter().map(|(whole, _)| *whole).collect(),
        }),
        // The pin's PEI: the call frame is reserved, or each call adjusts.
        Box::new(crate::backend::callframe::ReserveCallFrame {
            stack: registers.stack,
            width: registers.slot as u32,
            frame: frame.clone(),
        }),
        Box::new(prologue::Prologue::new(or_empty(), calls.cloned())),
        // After frame finalization, as the pin: a constant loaded again where
        // the allocator or rematerialization left it.
        Box::new(crate::backend::latecleanup::LateCleanup),
        Box::new(peephole::Peephole::with_rules(
            frame.clone(),
            target,
            rules,
            registers.saved.iter().map(|(whole, _)| *whole).collect(),
            Rc::clone(classes),
            selector,
        )?),
        // After the peephole, as the pin runs it late (X86FixupBWInsts).
        Box::new(crate::backend::fixupbw::FixupBw { size: target.size }),
        Box::new(crate::backend::fixupleas::FixupLeas {
            slow_3ops: target.slow_3ops_lea(),
            size: target.size,
            ..Default::default()
        }),
        // Once spill traffic is final: which slots a loop still reaches.
        Box::new(loopslots::LoopSlots::new(frame.clone(), target, registers.slot as u32, classes)?),
        // Scheduling may only move fully allocated machine occurrences.
        Box::new(schedule::Scheduler::new(target)?),
        // Last: this physical order decides which explicit edge is now
        // fall-through.
        Box::new(jumps::ControlFlow { cpu: target, calls: calls.cloned() }),
        // The pin runs MachineCopyPropagation again after tail duplication.
        Box::new(crate::backend::machinecp::CopyPropagation),
    ];
    // -O0: gcc and clang improve nothing after selection and allocation.
    if !target.optimized {
        phases.retain(|phase| !UNOPTIMIZED.contains(&phase.class_name()));
    }
    // Under RFP the stackifier runs after allocation, on the FP
    // registers greedy assigned, as X86FloatingPoint does.
    if let Some(fps) = phases.iter().position(|phase| phase.class_name() == "FloatAlloc") {
        let moved = phases.remove(fps);
        let after = phases.iter().position(|phase| phase.class_name() == "RegAlloc").map_or(phases.len(), |at| at + 1);
        phases.insert(after, moved);
    }
    if cfg!(debug_assertions) {
        // At -O0 the optional phases are not run; a slot that is Translated
        // must be, and none of those is.
        let slot_of = |class: &str| SLOTS.iter().find(|(one, _)| *one == class).map_or("unknown", |(_, slot)| *slot);
        let built: Vec<&str> = phases.iter().map(|phase| slot_of(phase.class_name())).collect();
        let off: Vec<&str> =
            if target.optimized { Vec::new() } else { UNOPTIMIZED.iter().map(|one| slot_of(one)).collect() };
        llrm_transforms::parity::check_backend(&built, &off)?;
    }
    Ok(phases)
}

/// Return a well-formed body or name the phase boundary that is not.
pub fn verified(
    body: LirBody,
    stage: &str,
    in_ssa: bool,
    allocated: Option<&std::collections::BTreeSet<RegId>>,
    spaces: &llrm_mir::spaces::Spaces,
) -> Result<LirBody, Malformed> {
    let mut complaints = verify::verify_for(&body, in_ssa, spaces);
    if let Some(arguments) = allocated {
        complaints.extend(verify::physical(&body, arguments));
    }
    if let Some(first) = complaints.first() {
        return Err(Malformed(format!("{stage}: {first}")));
    }
    Ok(body)
}

/// A phase's refusal, or the malformed body it returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Checked {
    Refused(crate::model::passes::Exception),
    Malformed(Malformed),
}

/// Run one machine phase and verify what it returned.
pub fn checked(
    body: LirBody,
    phase: &mut dyn LIRTransform,
    in_ssa: bool,
    allocated: Option<&std::collections::BTreeSet<RegId>>,
    classes: &crate::backend::classes::RegisterClasses,
    segments: &Segments,
    arch: &dyn llrm_target::Target,
) -> Result<LirBody, Checked> {
    let stage = if phase.name().is_empty() { phase.class_name().to_owned() } else { phase.name().to_owned() };
    // The invariance instrument, LLVM's `-g` rule: stripped of meta
    // instructions, every phase must make the same code.
    let body = if llrm_support::env_flag!("LLRM_STRIP_META") { without_meta(body) } else { body };
    let verifying = crate::support::debug::verifying();
    let owned =
        if verifying { crate::support::debug::timed("lir owned bytes", || body.owned_bytes()) } else { Vec::new() };
    let transformed = crate::support::debug::timed_by(|| format!("lir {stage}"), || phase.transform_raising(body))
        .map_err(Checked::Refused)?;
    transformed.facts.0.end_phase();
    let body = if verifying {
        crate::support::debug::timed("lir verify", || {
            verified(transformed, &stage, in_ssa, allocated, &arch.layout().spaces.roles)
        })
        .map_err(Checked::Malformed)?
    } else {
        transformed
    };
    if crate::support::debug::enabled("regclass")
        && matches!(
            stage.as_str(),
            "PhiElimination"
                | "phielim"
                | "FloatAssign"
                | "FloatAlloc"
                | "TwoAddress"
                | "twoaddr"
                | "Coalescer"
                | "coalesce"
        )
    {
        let found = crate::backend::regclass::violations(
            &body,
            segments,
            classes,
            &crate::backend::regclass::untouchable(&body),
        );
        let peak = found
            .iter()
            .filter_map(|one| {
                if let crate::backend::regclass::Why::Crowded { live, registers } = one.why {
                    Some(live - registers)
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0);
        let blocks: std::collections::BTreeSet<i64> = found.iter().map(|one| one.block).collect();
        llrm_support::debug!(
            "regclass",
            "{} after {stage}: {} points do not fit, peak {peak} over, {} blocks",
            body.name,
            found.len(),
            blocks.len()
        );
    }
    if !verifying {
        return Ok(body);
    }
    let now = crate::support::debug::timed("lir owned bytes", || body.owned_bytes());
    if now != owned {
        let (lost, gained) = (difference(&owned, &now), difference(&now, &owned));
        let listed = |bytes: Vec<i64>| bytes.iter().map(|one| format!("{one:#x}")).collect::<Vec<_>>().join(" ");
        return Err(Checked::Malformed(Malformed(format!(
            "{stage}: lost source bytes [{}], gained [{}]",
            listed(lost),
            listed(gained)
        ))));
    }
    Ok(body)
}

/// `body` without meta instructions, and without the unreachable blocks
/// that held only those.
fn without_meta(body: LirBody) -> LirBody {
    let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &*block.succ)).collect();
    let (mut reached, mut work) = (std::collections::BTreeSet::new(), vec![body.entry]);
    while let Some(at) = work.pop() {
        if succ.contains_key(&at) && reached.insert(at) {
            work.extend(succ[&at].iter().copied());
        }
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| block.with_insns(block.insns.iter().filter(|one| !one.is_meta()).cloned().collect()))
        .filter(|block| reached.contains(&block.at) || !block.insns.is_empty() || !block.phis.is_empty())
        .collect();
    LirBody { blocks, ..body }
}

/// What sorted `one` has that sorted `other` lacks, counting repeats.
fn difference(
    one: &[i64],
    other: &[i64],
) -> Vec<i64> {
    let mut other = other.iter().peekable();
    one.iter()
        .filter(|&&byte| {
            while other.next_if(|&&next| next < byte).is_some() {}
            other.next_if_eq(&&byte).is_none()
        })
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{self, Loc, Operation};
    use crate::model::lir::{Insn, LirBlock};
    use crate::support::hash::IndexMap;

    struct LosesDefinition;

    impl LIRTransform for LosesDefinition {
        fn class_name(&self) -> &'static str {
            "LosesDefinition"
        }

        fn name(&self) -> &str {
            "loses-definition"
        }

        fn transform(
            &mut self,
            mut body: LirBody,
        ) -> Result<LirBody, String> {
            let mut broken = (*body.blocks[0].insns[0]).clone();
            broken.uses = vec![99];
            body.blocks[0].insns = vec![Arc::new(broken)].into();
            Ok(body)
        }
    }

    struct DropsBytes;

    impl LIRTransform for DropsBytes {
        fn class_name(&self) -> &'static str {
            "DropsBytes"
        }

        fn transform(
            &mut self,
            mut body: LirBody,
        ) -> Result<LirBody, String> {
            body.blocks[0]
                .insns
                .edit(
                    |insns| {
                        insns.remove(0);
                    },
                );
            Ok(body)
        }
    }

    /// jumps deleted a block holding only a source `jmp`, and its three
    /// bytes had no owner; nothing noticed.
    /// Under RFP floatassign's slot keeps its legalization only
    /// and the stackifier runs after allocation, on the FP registers greedy
    /// assigned (X86FloatingPoint's place); without it the order is as ever.
    #[test]
    fn test_the_stackifier_runs_after_allocation() {
        let names = || -> Vec<&'static str> {
            machine(&IndexMap::default(), None, None, None, false, "486", &crate::backend::target::BUILT_IN)
                .expect("phases")
                .iter()
                .map(|phase| phase.class_name())
                .collect()
        };
        let at = |names: &[&str], one: &str| names.iter().position(|name| *name == one);
        let after = names();
        assert_eq!(at(&after, "FloatAssign"), None);
        assert!(at(&after, "FloatPrep") < at(&after, "RegAlloc"), "{after:?}");
        assert_eq!(at(&after, "FloatAlloc"), at(&after, "RegAlloc").map(|one| one + 1), "{after:?}");
    }

    /// The pin folds a once-read load into `ADD32rm` in selection, before the
    /// combiner. Folded after it, the combiner saw `add rr` for a stack
    /// argument and rebuilt a six-term sum as a tree: `mov eax, edx` and a
    /// different register order in two llrm-c listings.
    #[test]
    fn test_a_load_is_folded_before_the_combiner_sees_the_chain() {
        let names: Vec<&'static str> =
            machine(&IndexMap::default(), None, None, None, false, "486", &crate::backend::target::BUILT_IN)
                .expect("phases")
                .iter()
                .map(|phase| phase.class_name())
                .collect();
        let at = |one: &str| names.iter().position(|name| *name == one);
        assert!(at("LoadFold").is_some() && at("LoadFold") < at("MachineCombiner"), "{names:?}");
    }

    /// The pin runs the MachineCombiner before EarlyMachineLICM, MachineCSE,
    /// the sink and the peephole (`llc -O2 -debug-pass`); ours ran after the
    /// peephole, so the chains it rebuilds were already rewritten.
    #[test]
    fn test_the_combiner_runs_before_licm_cse_sink_and_peephole() {
        let names: Vec<&'static str> =
            machine(&IndexMap::default(), None, None, None, false, "486", &crate::backend::target::BUILT_IN)
                .expect("phases")
                .iter()
                .map(|phase| phase.class_name())
                .collect();
        let at = |one: &str| names.iter().position(|name| *name == one).expect(one);
        for later in ["MachineLICM", "MachineCSE", "MachineSink", "PeepholeOptimizer"] {
            assert!(at("MachineCombiner") < at(later), "{later}: {names:?}");
        }
    }

    /// `llc -O2 -debug-pass` has X86FixupSetCC before the pre-RA scheduler;
    /// ours ran after it, so the zero a `setcc` needs was placed once the
    /// order was fixed (x_rle m32: a `lea` in another place).
    #[test]
    fn test_fixupsetcc_runs_before_the_machine_scheduler() {
        let names: Vec<&'static str> =
            machine(&IndexMap::default(), None, None, None, false, "486", &crate::backend::target::BUILT_IN)
                .expect("phases")
                .iter()
                .map(|phase| phase.class_name())
                .collect();
        let at = |one: &str| names.iter().position(|name| *name == one).expect(one);
        assert!(at("FixupSetCC") < at("MachineScheduler"), "{names:?}");
    }

    #[test]
    fn test_a_phase_that_loses_source_bytes_is_malformed() {
        let jump = ir::Semantics { name: Some("jmp".into()), target: Some(1), ..ir::Semantics::new(Operation::Jump) };
        let returned = ir::Semantics { name: Some("ret".into()), ..ir::Semantics::new(Operation::Return) };
        let block = LirBlock::new(
            1,
            vec![
                Arc::new(Insn::new(1, Some((1, 4)), Some(jump), vec![], vec![])),
                Arc::new(Insn::new(4, Some((4, 5)), Some(returned), vec![], vec![])),
            ],
        );
        let body = LirBody::new("bytes", 1, vec![block.into()], IndexMap::default(), IndexMap::default());
        let Err(Checked::Malformed(Malformed(said))) = checked(
            body,
            &mut DropsBytes,
            false,
            None,
            &crate::backend::classes::RegisterClasses::m16(),
            &crate::backend::target::BUILT_IN,
            &llrm_x86_m16::M16,
        ) else {
            panic!("the gate let three source bytes go");
        };
        assert_eq!(said, "DropsBytes: lost source bytes [0x1 0x2 0x3], gained []");
    }

    #[test]
    fn test_the_machine_phase_gate_names_the_phase_that_made_bad_lir() {
        let what = ir::Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(ir::Held { value: 2, width: 2 })],
            sources: vec![Loc::Held(ir::Held { value: 1, width: 2 })],
            ..ir::Semantics::new(Operation::Move)
        };
        let source = Insn::new(1, Some((1, 1)), Some(what), vec![2], vec![1]);
        let mut body = LirBody::new(
            "phase",
            1,
            vec![LirBlock::new(1, vec![Arc::new(source)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = BTreeSet::from([1]);
        let Err(Checked::Malformed(Malformed(said))) = checked(
            body,
            &mut LosesDefinition,
            false,
            None,
            &crate::backend::classes::RegisterClasses::m16(),
            &crate::backend::target::BUILT_IN,
            &llrm_x86_m16::M16,
        ) else {
            panic!("the gate let a lost definition through");
        };
        assert!(said.starts_with("loses-definition: value#99 is read"), "{said}");
    }
    /// A phase that changes nothing.
    struct Keeps;

    impl LIRTransform for Keeps {
        fn class_name(&self) -> &'static str {
            "Keeps"
        }

        fn transform(
            &mut self,
            body: LirBody,
        ) -> Result<LirBody, String> {
            Ok(body)
        }
    }

    /// The last answer of `intervals` stayed kept after the phase that asked
    /// for it, with a body of instructions each: 3.3 MB of the 24 MB peak of
    /// d_faces -O2. What a body's facts stash of it is let go when a phase
    /// ends.
    #[test]
    fn test_what_the_facts_stash_is_let_go_when_a_phase_ends() {
        let body =
            LirBody::new("f", 1, vec![LirBlock::new(1, vec![]).into()], IndexMap::default(), IndexMap::default());
        let kept = Arc::new(7_u32);
        body.facts.0.stash(|held: &mut Vec<Arc<u32>>| held.push(Arc::clone(&kept)));
        assert_eq!(Arc::strong_count(&kept), 2, "premise: the facts hold it");
        let body = checked(
            body,
            &mut Keeps,
            false,
            None,
            &crate::backend::classes::RegisterClasses::m16(),
            &crate::backend::target::BUILT_IN,
            &llrm_x86_m16::M16,
        )
        .ok()
        .expect("a phase that changes nothing");
        assert_eq!(Arc::strong_count(&kept), 1, "the phase's end kept it");
        drop(body);
    }

    /// Each PR that added a phase bumped the literal length of `UNOPTIMIZED`
    /// and `SLOTS`: two of them in one batch did not build, and the second
    /// always conflicted (the 18:10 batch, 12 PRs). The lists are slices.
    #[test]
    fn the_phase_lists_have_no_literal_length() {
        let text = include_str!("flow.rs");
        for name in ["UNOPTIMIZED", "SLOTS"] {
            let line = text.lines().find(|line| line.starts_with(&format!("const {name}:"))).expect("the list");
            assert!(line.contains(": &["), "{line}");
        }
    }

    /// Every phase `-O0` leaves out is a phase with a slot.
    #[test]
    fn every_unoptimized_phase_has_a_slot() {
        for class in UNOPTIMIZED {
            assert!(SLOTS.iter().any(|(one, _)| one == class), "{class}");
        }
    }
}
