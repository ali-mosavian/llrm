//! The machine phases and their gate: each LIR phase run, verified and checked.

use std::cell::RefCell;
use std::rc::Rc;

use crate::support::hash::IndexMap;
use iced_x86::Register;

use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::constpool::Pool;
use crate::backend::frame::Frame;
use crate::backend::target::Segments;
use crate::backend::{
    allocate, coalesce, farcall, floatalloc, floatassign, jumps, loopslots, parcopy, peephole, phielim, prologue, schedule, ssaspill, twoaddr,
};

use crate::backend::verify::{self, Malformed};
use crate::model::lir::LirBody;
use crate::model::passes::LIRTransform;


/// Every phase between instruction selection and emission, in order, with the
/// spiller in front of the allocator or left out.
#[allow(clippy::too_many_arguments)]
pub fn machine<'a>(
    pinned: &IndexMap<u32, Register>,
    frame: Option<Rc<RefCell<Frame>>>,
    pool: Option<Rc<RefCell<Pool>>>,
    calls: Option<&IndexMap<i64, String>>,
    basic_semantics: bool,
    cpu: impl Into<ProfileOrName<'a>>,
    segments: &Segments,
    spilling: bool,
) -> Result<Vec<Box<dyn LIRTransform + 'a>>, String> {
    machine_with(pinned, frame, pool, calls, basic_semantics, cpu, segments, spilling.then(Rc::<ssaspill::Run>::default), &crate::backend::peep::targets::x86_code16::RULES, &llrm_target::Target::frame_registers(&llrm_x86_code16::Code16))
}

/// `machine`, its peephole made of the rules `rules` holds; the spiller, where `spilling` names a run, reports to it.
#[allow(clippy::too_many_arguments)]
pub fn machine_with<'a>(
    pinned: &IndexMap<u32, Register>,
    frame: Option<Rc<RefCell<Frame>>>,
    pool: Option<Rc<RefCell<Pool>>>,
    calls: Option<&IndexMap<i64, String>>,
    basic_semantics: bool,
    cpu: impl Into<ProfileOrName<'a>>,
    segments: &Segments,
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
        Box::new(ssaspill::SsaSpill { frame: or_empty(), segments: segments.clone(), prices: ssaspill::Prices::of(target), run: spilling.clone().unwrap_or_default() }),
        Box::new(phielim::PhiElimination),
        // After phi elimination: a phi's copies are where the stack shuffles.
        Box::new(floatassign::FloatAssign { frame: frame.clone(), pool, basic_semantics, cpu: target }),
        Box::new(floatalloc::FloatAlloc { frame: frame.clone() }),
        Box::new(twoaddr::TwoAddress),
        Box::new(coalesce::Coalescer::new(None, segments)),
        Box::new(allocate::RegAlloc::new(Some(&pinned), frame.clone(), ProfileOrName::Profile(target), segments)?),
        // After allocation: which moves in a phi's copy conflict is a question about locations.
        Box::new(parcopy::ParallelCopy),
        Box::new(prologue::Prologue::new(or_empty(), calls.cloned())),
        Box::new(peephole::Peephole::with_rules(frame.clone(), target, rules, registers.saved.iter().map(|(whole, _)| *whole).collect())?),
        // Once spill traffic is final: which slots a loop still reaches.
        Box::new(loopslots::LoopSlots::new(frame.clone(), target)?),
        // Scheduling may only move fully allocated machine occurrences.
        Box::new(schedule::Scheduler::new(target)?),
        // Last: this physical order decides which explicit edge is now fall-through.
        Box::new(jumps::ControlFlow { cpu: target }),
    ];
    if spilling.is_none() {
        phases.retain(|phase| phase.class_name() != "SsaSpill");
    }
    Ok(phases)
}

/// Return a well-formed body or name the phase boundary that is not.
pub fn verified(body: LirBody, stage: &str, in_ssa: bool) -> Result<LirBody, Malformed> {
    let complaints = verify::verify(&body, in_ssa);
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
pub fn checked(body: LirBody, phase: &mut dyn LIRTransform, in_ssa: bool) -> Result<LirBody, Checked> {
    let stage = if phase.name().is_empty() { phase.class_name().to_owned() } else { phase.name().to_owned() };
    // The invariance instrument, LLVM's `-g` rule: stripped of meta
    // instructions, every phase must make the same code.
    let body = if std::env::var_os("LLRM_STRIP_META").is_some() { without_meta(body) } else { body };
    let owned = body.owned_bytes();
    let transformed =
        crate::support::debug::timed(&format!("lir {stage}"), || phase.transform_raising(body)).map_err(Checked::Refused)?;
    let body = verified(transformed, &stage, in_ssa).map_err(Checked::Malformed)?;
    if crate::support::debug::enabled("regclass") && matches!(stage.as_str(), "SsaSpill" | "ssaspill" | "PhiElimination" | "phielim" | "FloatAssign" | "FloatAlloc" | "TwoAddress" | "twoaddr" | "Coalescer" | "coalesce") {
        let found = crate::backend::regclass::violations(&body, &crate::backend::target::BUILT_IN, &crate::backend::ssaspill::untouchable(&body));
        let peak = found.iter().filter_map(|one| if let crate::backend::regclass::Why::Crowded { live, registers } = one.why { Some(live - registers) } else { None }).max().unwrap_or(0);
        let blocks: std::collections::BTreeSet<i64> = found.iter().map(|one| one.block).collect();
        llrm_support::debug!("regclass", "{} after {stage}: {} points do not fit, peak {peak} over, {} blocks", body.name, found.len(), blocks.len());
    }
    let now = body.owned_bytes();
    if now != owned {
        let (lost, gained) = (difference(&owned, &now), difference(&now, &owned));
        let listed = |bytes: Vec<i64>| bytes.iter().map(|one| format!("{one:#x}")).collect::<Vec<_>>().join(" ");
        return Err(Checked::Malformed(Malformed(format!("{stage}: lost source bytes [{}], gained [{}]", listed(lost), listed(gained)))));
    }
    Ok(body)
}

/// `body` without meta instructions, and without the unreachable blocks
/// that held only those.
fn without_meta(body: LirBody) -> LirBody {
    let succ: IndexMap<i64, &Vec<i64>> = body.blocks.iter().map(|block| (block.at, &block.succ)).collect();
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
fn difference(one: &[i64], other: &[i64]) -> Vec<i64> {
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

    use crate::support::hash::IndexMap;

    use super::*;
    use crate::model::ir::{self, Loc, Operation};
    use crate::model::lir::{Insn, LirBlock};

    struct LosesDefinition;

    impl LIRTransform for LosesDefinition {
        fn class_name(&self) -> &'static str {
            "LosesDefinition"
        }

        fn name(&self) -> &str {
            "loses-definition"
        }

        fn transform(&mut self, mut body: LirBody) -> Result<LirBody, String> {
            let mut broken = (*body.blocks[0].insns[0]).clone();
            broken.uses = vec![99];
            body.blocks[0].insns = vec![Arc::new(broken)];
            Ok(body)
        }
    }

    struct DropsBytes;

    impl LIRTransform for DropsBytes {
        fn class_name(&self) -> &'static str {
            "DropsBytes"
        }

        fn transform(&mut self, mut body: LirBody) -> Result<LirBody, String> {
            body.blocks[0].insns.remove(0);
            Ok(body)
        }
    }

    /// jumps deleted a block holding only a source `jmp`, and its three
    /// bytes had no owner; nothing noticed.
    #[test]
    fn test_a_phase_that_loses_source_bytes_is_malformed() {
        let jump = ir::Semantics { name: Some("jmp".into()), target: Some(1), ..ir::Semantics::new(Operation::Jump) };
        let returned = ir::Semantics { name: Some("ret".into()), ..ir::Semantics::new(Operation::Return) };
        let block = LirBlock::new(1, vec![Arc::new(Insn::new(1, Some((1, 4)), Some(jump), vec![], vec![])), Arc::new(Insn::new(4, Some((4, 5)), Some(returned), vec![], vec![]))]);
        let body = LirBody::new("bytes", 1, vec![block], IndexMap::default(), IndexMap::default());
        let Err(Checked::Malformed(Malformed(said))) = checked(body, &mut DropsBytes, false) else {
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
        let mut body =
            LirBody::new("phase", 1, vec![LirBlock::new(1, vec![Arc::new(source)])], IndexMap::default(), IndexMap::default());
        body.inputs = BTreeSet::from([1]);
        let Err(Checked::Malformed(Malformed(said))) = checked(body, &mut LosesDefinition, false) else {
            panic!("the gate let a lost definition through");
        };
        assert!(said.starts_with("loses-definition: value#99 is read"), "{said}");
    }
}
