//! Port of `qbopt/flow.py`: so far the MIR fixed point, the machine phases and their gate.

use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use crate::support::hash::IndexMap;
use iced_x86::Register;

use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::frame::Frame;
use crate::backend::{
    allocate, coalesce, farcall, floatalloc, jumps, parcopy, peephole, phielim, prologue, schedule, twoaddr,
};

use crate::backend::verify::{self, Malformed};
use crate::model::lir::LirBody;
use crate::model::mir::MirBody;
use crate::model::passes::{LIRTransform, Options, LEVELS};
use crate::optimize::transform;

/// Every phase between lowering and emission, in order.
pub fn machine<'a>(
    pinned: &IndexMap<u32, Register>,
    frame: Option<Rc<RefCell<Frame>>>,
    calls: Option<&IndexMap<i64, String>>,
    basic_semantics: bool,
    cpu: impl Into<ProfileOrName<'a>>,
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
    Ok(vec![
        Box::new(farcall::FarIndirectCalls::new(or_empty())),
        Box::new(floatalloc::FloatAlloc::new(frame.clone(), basic_semantics, target)?),
        Box::new(phielim::PhiElimination),
        Box::new(twoaddr::TwoAddress),
        Box::new(coalesce::Coalescer::new(None)),
        Box::new(allocate::RegAlloc::new(Some(&pinned), frame.clone(), ProfileOrName::Profile(target))?),
        // After allocation: which moves in a phi's copy conflict is a question about locations.
        Box::new(parcopy::ParallelCopy),
        Box::new(prologue::Prologue::new(or_empty(), calls.cloned())),
        Box::new(peephole::Peephole::new(frame.clone(), target)?),
        // Scheduling may only move fully allocated machine occurrences.
        Box::new(schedule::Scheduler::new(target)?),
        // Last: this physical order decides which explicit edge is now fall-through.
        Box::new(jumps::ControlFlow),
    ])
}

/// The MIR fixed point every driver runs, configured by target and options alone.
///
/// A switch one frontend sets and another does not makes the same program
/// compile differently by spelling: sum_three took three paths here.
/// Promotion needs dominators, which an irreducible CFG -- QB's RESUME
/// entering a loop -- does not have; that is a fact about the body.
#[allow(clippy::too_many_arguments)]
pub fn optimized<'a>(
    body: &Rc<MirBody>,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    cpu: impl Into<ProfileOrName<'a>>,
    options: Options,
    blocks: Option<Rc<Vec<crate::frontends::bc::blocks::Block>>>,
    found: Option<Rc<crate::objectfile::module::Module>>,
    only: Option<String>,
    watch: Option<&mut dyn FnMut(&str, &MirBody)>,
) -> Result<Rc<MirBody>, String> {
    use crate::analysis::loops;

    let target = targets::profile(cpu)?;
    let mut options = options;
    if !loops::irreducible(&body.blocks, Some(body.entry)).is_empty() {
        options = Options { promote: false, ..options };
    }
    transform::applied(
        body,
        dgroup,
        calls,
        transform::Applied {
            blocks,
            found,
            only,
            options,
            registers: Some(target.register_capacity),
            call_registers: target.call_register_capacity,
            index_scales: Some(target.address_scales.clone()),
            address_forms: Some(target.address_forms.clone()),
            costs: Some(target.operations.clone()),
            watch,
        },
    )
}

/// GCC's spelling, `-Os` or `-O2`: `level_option`'s `named`.
pub fn level_option(text: &str) -> Result<Options, String> {
    LEVELS()
        .get(format!("O{text}").as_str())
        .cloned()
        .ok_or_else(|| format!("unknown level -O{text}; choose -Os or -O2"))
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
    let owned = body.owned_bytes();
    let transformed =
        crate::support::debug::timed(&format!("lir {stage}"), || phase.transform_raising(body)).map_err(Checked::Refused)?;
    let body = verified(transformed, &stage, in_ssa).map_err(Checked::Malformed)?;
    let now = body.owned_bytes();
    if now != owned {
        let (lost, gained) = (difference(&owned, &now), difference(&now, &owned));
        let listed = |bytes: Vec<i64>| bytes.iter().map(|one| format!("{one:#x}")).collect::<Vec<_>>().join(" ");
        return Err(Checked::Malformed(Malformed(format!("{stage}: lost source bytes [{}], gained [{}]", listed(lost), listed(gained)))));
    }
    if stage == "jumps" && crate::support::debug::enabled("cost") {
        llrm_support::debug!("cost", "{}", crate::backend::executed::summary(&body));
    }
    Ok(body)
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
