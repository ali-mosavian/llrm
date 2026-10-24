//! `ProcessImplicitDefs`: the uses of an undefined value are flagged undef and
//! the definition goes. Translated from
//! llvm/lib/CodeGen/ProcessImplicitDefs.cpp at d1106deb71cc
//! (`processImplicitDef`, `canTurnIntoImplicitDef`).
//!
//! # How it works
//! An `implicit_def` defines one value and reads nothing. Each instruction that
//! reads that value has it moved from `uses` to `undefs` (`MO.setIsUndef()`):
//! no live range reaches the read and the allocator gives the operand any
//! register. A copy left reading nothing else is an `implicit_def` of its own
//! destination and is worked in turn. A definition nothing reads any more is
//! erased; one a phi still reads stays.
//!
//! DEVIATE PI-1 (no number): no physical-register implicit defs; the target has
//! none before allocation.
//!
//! DEVIATE PI-3 (exit: none): only a full copy is turned into an
//! `implicit_def`; the pin also turns `INSERT_SUBREG` and `REG_SEQUENCE`, which
//! LIR has not.
//!
//! # Example
//! `v1 := implicit_def; v2 := mov v1; v3 := add v4, v2`: `v2` is an
//! `implicit_def` too, and `v3 := add v4, v2` reads `v2` as undef; both
//! definitions go.
//!
//! A definition a phi still reads stays: PHIElimination takes a phi of such
//! values for an `implicit_def` and makes no copy of one
//! (`allPhiOperandsUndefined`, `SrcUndef`).
//!
//! # Used by
//! `flow::machine_with`, after `SharedConstants`; `isel`'s `held` makes the
//! `implicit_def`s of a poison or undef value a register holds.

use std::sync::Arc;

use crate::coalescer::pair::full_copy;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashSet;

pub const IMPLICIT_DEF: &str = "implicit_def";

/// `one` defines a value and nothing else: an `implicit_def`.
pub(crate) fn implicit_def_of(one: &Insn) -> Option<u32> {
    let what = one.what.as_ref()?;
    (what.name.as_deref() == Some(IMPLICIT_DEF) && one.defines.len() == 1 && one.uses.is_empty())
        .then(|| one.defines[0])
}

/// The `implicit_def` that defines `value`, in the place of the copy `one`.
fn made_implicit(
    one: &Insn,
    value: u32,
) -> Arc<Insn> {
    let what = one.what.as_ref().expect("a copy");
    let dest = what.dests[0].clone();
    let mut made = one.clone();
    made.what = Some(Semantics {
        op: Operation::Nothing,
        name: Some(IMPLICIT_DEF.into()),
        dests: vec![dest],
        sources: Vec::new(),
        ..Semantics::new(Operation::Nothing)
    });
    made.defines = vec![value];
    made.uses = Vec::new();
    made.undefs = Vec::new();
    Arc::new(made)
}

/// The pass as a phase: what a phi reads stays an `implicit_def`, for
/// PHIElimination.
pub struct ProcessImplicitDefs;

impl LIRTransform for ProcessImplicitDefs {
    fn class_name(&self) -> &'static str {
        "ProcessImplicitDefs"
    }

    fn name(&self) -> &str {
        "processimpdefs"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        if !body.blocks.iter().flat_map(|block| block.insns.iter()).any(|one| implicit_def_of(one).is_some()) {
            return Ok(body);
        }
        Ok(processed(&body))
    }
}

/// `body` with every `implicit_def` processed.
pub fn processed(body: &LirBody) -> LirBody {
    let mut current = body.clone();
    loop {
        let defined: HashSet<u32> =
            current.blocks.iter().flat_map(|block| block.insns.iter()).filter_map(|one| implicit_def_of(one)).collect();
        if defined.is_empty() {
            return current;
        }
        let phi_read: HashSet<u32> = current
            .blocks
            .iter()
            .flat_map(|block| block.phis.iter())
            .flat_map(|phi| phi.incoming.iter().map(|&(_, value)| value))
            .collect();
        let mut changed = false;
        let blocks: Vec<_> = current
            .blocks
            .iter()
            .map(|block| {
                let insns: Vec<Arc<Insn>> = block
                    .insns
                    .iter()
                    .map(|one| {
                        let flagged: Vec<u32> = one.uses.iter().copied().filter(|v| defined.contains(v)).collect();
                        if flagged.is_empty() {
                            return Arc::clone(one);
                        }
                        changed = true;
                        let mut made = (**one).clone();
                        made.uses.retain(|value| !flagged.contains(value));
                        // The register it was to arrive in is no longer asked
                        // for: the value is any.
                        made.requires.retain(|(held, _)| !flagged.contains(&held.value));
                        made.undefs.extend(flagged.iter().copied());
                        // A copy that now reads nothing is an implicit def
                        // itself.
                        if made.uses.is_empty()
                            && let Some((dst, _)) = full_copy(one)
                        {
                            return made_implicit(&made, dst.value);
                        }
                        Arc::new(made)
                    })
                    .collect();
                block.with_insns(insns)
            })
            .collect();
        // A definition nothing but undef reads reads it goes, one a phi reads
        // stays.
        let before: usize = blocks.iter().map(|block| block.insns.len()).sum();
        let blocks: Vec<_> = blocks
            .into_iter()
            .map(|block| {
                let insns: Vec<Arc<Insn>> = block
                    .insns
                    .iter()
                    .filter(|one| {
                        implicit_def_of(one).is_none_or(|value| phi_read.contains(&value) || !defined.contains(&value))
                    })
                    .cloned()
                    .collect();
                block.with_insns(insns)
            })
            .collect();
        let after: usize = blocks.iter().map(|block| block.insns.len()).sum();
        if !changed && after == before {
            return current;
        }
        current = current.with_blocks(blocks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Held, Loc};
    use crate::model::lir::{LirBlock, Phi};
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn def(value: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some(IMPLICIT_DEF.into()),
            dests: vec![held(value)],
            ..Semantics::new(Operation::Nothing)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![value], Vec::new()))
    }

    fn copy(
        into: u32,
        from: u32,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![held(into)],
            sources: vec![held(from)],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![into], vec![from]))
    }

    fn add(
        into: u32,
        first: u32,
        second: u32,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![held(into)],
            sources: vec![held(first), held(second)],
            ..Semantics::new(Operation::Binary)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![into], vec![first, second]))
    }

    fn body(
        insns: Vec<Arc<Insn>>,
        phis: Vec<Phi>,
    ) -> LirBody {
        let block = LirBlock { phis: phis.into(), ..LirBlock::new(0, insns) };
        LirBody::new("t", 0, vec![block.into()], IndexMap::default(), IndexMap::default())
    }

    fn names(body: &LirBody) -> Vec<String> {
        body.insns().iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    /// The use becomes an undef read and the definition goes.
    #[test]
    fn test_a_use_of_an_implicit_def_is_flagged_undef_and_the_def_is_erased() {
        let got = processed(&body(vec![def(1), add(3, 4, 1)], Vec::new()));
        assert_eq!(names(&got), ["add"]);
        let add = &got.insns()[0];
        assert_eq!((add.uses.clone(), add.undefs.clone()), (vec![4], vec![1]));
    }

    /// A copy that reads only undef is an implicit def, worked in turn: `v2 :=
    /// mov v1` then `v3 := add v4, v2`.
    #[test]
    fn test_a_copy_of_an_implicit_def_is_an_implicit_def_and_its_users_read_undef() {
        let got = processed(&body(vec![def(1), copy(2, 1), add(3, 4, 2)], Vec::new()));
        assert_eq!(names(&got), ["add"]);
        assert_eq!(got.insns()[0].undefs, vec![2]);
    }

    /// An instruction that reads something else too keeps reading it, and is no
    /// implicit def.
    #[test]
    fn test_an_instruction_with_another_read_is_not_an_implicit_def() {
        let got = processed(&body(vec![def(1), add(3, 4, 1)], Vec::new()));
        assert_eq!(got.insns()[0].what.as_ref().unwrap().name.as_deref(), Some("add"));
    }

    /// A definition nothing reads goes too (`AllUsesUndef` is true of no uses).
    #[test]
    fn test_an_implicit_def_nothing_reads_is_erased() {
        let got = processed(&body(vec![def(1), add(3, 4, 5)], Vec::new()));
        assert_eq!(names(&got), ["add"]);
    }

    /// A phi that reads the value keeps the definition: PHIElimination takes
    /// it.
    #[test]
    fn test_a_definition_a_phi_reads_stays() {
        let phi = Phi { result: 5, incoming: vec![(0, 1)] };
        let got = processed(&body(vec![def(1), add(3, 4, 1)], vec![phi]));
        assert_eq!(names(&got), ["implicit_def", "add"]);
    }
}
