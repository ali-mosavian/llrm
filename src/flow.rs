//! Port of `qbopt/flow.py`: so far the phase gate.

use crate::backend::verify::{self, Malformed};
use crate::model::lir::LirBody;
use crate::model::passes::LIRTransform;

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
    Refused(String),
    Malformed(Malformed),
}

/// Run one machine phase and verify what it returned.
pub fn checked(body: LirBody, phase: &mut dyn LIRTransform, in_ssa: bool) -> Result<LirBody, Checked> {
    let transformed = phase.transform(body).map_err(Checked::Refused)?;
    let stage = if phase.name().is_empty() { phase.class_name().to_owned() } else { phase.name().to_owned() };
    verified(transformed, &stage, in_ssa).map_err(Checked::Malformed)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use indexmap::IndexMap;

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
            LirBody::new("phase", 1, vec![LirBlock::new(1, vec![Arc::new(source)])], IndexMap::new(), IndexMap::new());
        body.inputs = BTreeSet::from([1]);
        let Err(Checked::Malformed(Malformed(said))) = checked(body, &mut LosesDefinition, false) else {
            panic!("the gate let a lost definition through");
        };
        assert!(said.starts_with("loses-definition: value#99 is read"), "{said}");
    }
}
