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

/// A body's pins, by value id, which is what the allocator is keyed on.
///
/// Python reads `getattr(body, "pins", None)`: only a raised body has pins,
/// so the caller passes them, or `None` for a plain `MirBody`.
pub fn _pinned(
    pins: Option<&crate::model::mir::OrderedMap<crate::model::mir::Value, iced_x86::Register>>,
) -> indexmap::IndexMap<u32, iced_x86::Register> {
    pins.map(|pins| pins.iter().map(|(value, register)| (value.id, *register)).collect())
        .unwrap_or_default()
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

    #[test]
    fn pinned_keys_a_raised_body_by_value_id() {
        use crate::model::mir::{OrderedMap, Value};
        use iced_x86::Register;

        let mut pins = OrderedMap::new();
        pins.insert(Value::new(7, 0x10), Register::SI);
        pins.insert(Value::new(3, 0x12), Register::DI);
        assert_eq!(_pinned(Some(&pins)), IndexMap::from([(7, Register::SI), (3, Register::DI)]));
        assert!(_pinned(None).is_empty());
    }
}
