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
    allocate, coalesce, farcall, floatalloc, floatassign, jumps, loopslots, parcopy, peephole, phielim, prologue,
    schedule, ssaspill, twoaddr,
};
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
    spilling: bool,
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
        spilling.then(Rc::<ssaspill::Run>::default),
        &crate::backend::targets::x86_m16::RULES,
        &llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
    )
}

/// The phases -O0 leaves out: each only makes code better.
const UNOPTIMIZED: [&str; 6] = ["Peephole", "LoopSlots", "Scheduler", "Coalescer", "PressureSink", "MachineSink"];

/// The parity registry's slot of each machine phase, by class name.
const SLOTS: [(&str, &str); 19] = [
    ("FarIndirectCalls", "farcall@1"),
    ("SharedConstants", "phiconst@2"),
    ("SsaSpill", "ssaspill@3"),
    ("PhiElimination", "phielim@4"),
    ("MachineSink", "machinesink@4"),
    ("PressureSink", "pressuresink@5"),
    ("FloatFold", "floatfold@6"),
    ("FloatAssign", "floatassign@7"),
    ("FloatPrep", "floatassign@7"),
    ("FloatAlloc", "floatalloc@8"),
    ("TwoAddress", "twoaddr@9"),
    ("Coalescer", "coalesce@10"),
    ("RegAlloc", "allocate@11"),
    ("ParallelCopy", "parcopy@12"),
    ("Prologue", "prologue@13"),
    ("Peephole", "peephole@14"),
    ("LoopSlots", "loopslots@15"),
    ("Scheduler", "schedule@16"),
    ("ControlFlow", "jumps@17"),
];

/// `machine`, its peephole made of the rules `rules` holds; the spiller, where
/// `spilling` names a run, reports to it.
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
    spilling: Option<Rc<ssaspill::Run>>,
    rules: &'static crate::backend::peep::Rules,
    registers: &llrm_target::FrameRegisters,
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
        Box::new(farcall::FarIndirectCalls::new(or_empty())),
        // Before SsaSpill: a constant several edges of a phi carry is one
        // value.
        Box::new(crate::backend::phiconst::SharedConstants),
        Box::new(ssaspill::SsaSpill {
            frame: or_empty(),
            segments: segments.clone(),
            classes: Rc::clone(classes),
            prices: ssaspill::Prices::of(target),
            run: spilling.clone().unwrap_or_default(),
        }),
        // On SSA, where a def with phi users alone can go to its edge.
        Box::new(crate::backend::machinesink::MachineSink { size: target.size }),
        Box::new(phielim::PhiElimination { prices: phielim::EdgePrices::of(target) }),
        // Before any value is placed: a load made where its reader is.
        Box::new(crate::backend::pressuresink::PressureSink {
            segments: segments.clone(),
            classes: Rc::clone(classes),
        }),
        // A float load only one arithmetic instruction reads is its memory
        // operand.
        Box::new(crate::backend::floatfold::FloatFold { cpu: target }),
        // After phi elimination: a phi's copies are where the stack shuffles.
        if crate::backend::allocate::rfp_on() {
            Box::new(floatassign::FloatPrep {
                frame: frame.clone(),
                pool,
                basic_semantics,
                status_word: classes.status_word(),
            }) as Box<dyn LIRTransform + 'a>
        } else {
            Box::new(floatassign::FloatAssign { frame: frame.clone(), pool, basic_semantics, cpu: target })
        },
        Box::new(floatalloc::FloatAlloc { frame: frame.clone(), classes: Rc::clone(classes) }),
        Box::new(twoaddr::TwoAddress),
        Box::new(coalesce::Coalescer { size: target.size, ..coalesce::Coalescer::new(None, segments, classes) }),
        Box::new(allocate::RegAlloc::new(
            Some(&pinned),
            frame.clone(),
            ProfileOrName::Profile(target),
            segments,
            classes,
        )?),
        // After allocation: which moves in a phi's copy conflict is a question
        // about locations.
        Box::new(parcopy::ParallelCopy {
            prices: parcopy::CyclePrices::of(target),
            available: classes.available.clone(),
            saved: registers.saved.iter().map(|(whole, _)| *whole).collect(),
        }),
        Box::new(prologue::Prologue::new(or_empty(), calls.cloned())),
        Box::new(peephole::Peephole::with_rules(
            frame.clone(),
            target,
            rules,
            registers.saved.iter().map(|(whole, _)| *whole).collect(),
            Rc::clone(classes),
        )?),
        // Once spill traffic is final: which slots a loop still reaches.
        Box::new(loopslots::LoopSlots::new(frame.clone(), target, registers.slot as u32, classes)?),
        // Scheduling may only move fully allocated machine occurrences.
        Box::new(schedule::Scheduler::new(target)?),
        // Last: this physical order decides which explicit edge is now
        // fall-through.
        Box::new(jumps::ControlFlow { cpu: target }),
    ];
    if spilling.is_none() {
        phases.retain(|phase| phase.class_name() != "SsaSpill");
    }
    // -O0: gcc and clang improve nothing after selection and allocation.
    if !target.optimized {
        phases.retain(|phase| !UNOPTIMIZED.contains(&phase.class_name()));
    }
    // Under `LLRM_GREEDY_RFP` the stackifier runs after allocation, on the FP
    // registers greedy assigned, as X86FloatingPoint does.
    if crate::backend::allocate::rfp_on()
        && let Some(fps) = phases.iter().position(|phase| phase.class_name() == "FloatAlloc")
    {
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
    spaces: &llrm_mir::spaces::Spaces,
) -> Result<LirBody, Malformed> {
    let complaints = verify::verify_for(&body, in_ssa, spaces);
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
            verified(transformed, &stage, in_ssa, &arch.layout().spaces.roles)
        })
        .map_err(Checked::Malformed)?
    } else {
        transformed
    };
    if crate::support::debug::enabled("regclass")
        && matches!(
            stage.as_str(),
            "SsaSpill"
                | "ssaspill"
                | "PhiElimination"
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
            &crate::backend::ssaspill::untouchable(&body),
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
    /// Under `LLRM_GREEDY_RFP` floatassign's slot keeps its legalization only
    /// and the stackifier runs after allocation, on the FP registers greedy
    /// assigned (X86FloatingPoint's place); without it the order is as ever.
    #[test]
    fn test_the_stackifier_runs_after_allocation_under_rfp() {
        let names = || -> Vec<&'static str> {
            machine(&IndexMap::default(), None, None, None, false, "486", &crate::backend::target::BUILT_IN, false)
                .expect("phases")
                .iter()
                .map(|phase| phase.class_name())
                .collect()
        };
        let at = |names: &[&str], one: &str| names.iter().position(|name| *name == one);
        let before = names();
        assert!(at(&before, "FloatAlloc") < at(&before, "RegAlloc") && at(&before, "FloatAssign").is_some());
        let _rfp = crate::backend::allocate::RfpOn::set();
        let after = names();
        assert_eq!(at(&after, "FloatAssign"), None);
        assert_eq!(at(&after, "FloatPrep"), at(&before, "FloatAssign"));
        assert_eq!(at(&after, "FloatAlloc"), at(&after, "RegAlloc").map(|one| one + 1), "{after:?}");
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
            &crate::backend::classes::RegisterClasses::m16(),
            &crate::backend::target::BUILT_IN,
            &llrm_x86_m16::M16,
        )
        .ok()
        .expect("a phase that changes nothing");
        assert_eq!(Arc::strong_count(&kept), 1, "the phase's end kept it");
        drop(body);
    }
}
