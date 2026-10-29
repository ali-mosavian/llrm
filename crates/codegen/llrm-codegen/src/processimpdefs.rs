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
//! DEVIATE PI-4 (exit: none while the core runs in order): an instruction that
//! reads a value it ignores reads it undefined, where the pin leaves a zero in
//! it to break the wait for the old register (`sbb r, r` after `MOV32r0`). A
//! core that runs in order does not wait, so `v1 := mov 0; v2 := sbb v1, v1`
//! is `v2 := sbb undef, undef`, and `v2` is free to be where it is wanted
//! (`cmp ax, 1; sbb ax, ax`, not `sbb ecx, ecx; mov ax, cx`). The pin does it
//! after allocation: `X86InstrInfo::expandPostRAPseudo` expands `SETB_C32r`
//! with `Expand2AddrUndef`, which marks the source undef.
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
//! A phi that reads nothing but `implicit_def`s is one too
//! (`canTurnIntoImplicitDef`), placed first in its block, and its users read
//! undef in turn. A definition a phi with a real operand still reads stays:
//! PHIElimination makes no copy of that operand (`allPhiOperandsUndefined`,
//! `SrcUndef`).
//!
//! # Used by
//! `flow::machine_with`, after `SharedConstants`; `isel`'s `held` makes the
//! `implicit_def`s of a poison or undef value a register holds.

use std::sync::Arc;

use crate::coalescer::pair::full_copy;
use crate::model::ir::{Held, Loc, Operation, Semantics};
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

/// `phi` reads only values that are `implicit_def`s.
fn all_undefined(
    phi: &crate::model::lir::Phi,
    defined: &HashSet<u32>,
) -> bool {
    !phi.incoming.is_empty() && phi.incoming.iter().all(|&(_, value)| defined.contains(&value))
}

/// `source`, an `implicit_def`, as the definition of `value`.
fn made_defining(
    source: &Insn,
    value: u32,
) -> Arc<Insn> {
    let mut made = source.clone();
    if let Some(what) = made.what.as_mut() {
        what.dests = what
            .dests
            .iter()
            .map(|dest| match dest {
                Loc::Held(held) => Loc::Held(Held { value, ..*held }),
                other => other.clone(),
            })
            .collect();
    }
    made.defines = vec![value];
    Arc::new(made)
}

/// The pass as a phase: what a phi reads stays an `implicit_def`, for
/// PHIElimination. With `ignored_inputs` a constant only an instruction that
/// ignores it reads is one too (PI-4).
pub struct ProcessImplicitDefs {
    pub ignored_inputs: bool,
}

/// `one` is `sbb r, r`: its result is the borrow whatever `r` holds. The value
/// it reads.
fn ignores_its_input(one: &Insn) -> Option<u32> {
    let value = *one.uses.first()?;
    let what = one.what.as_ref()?;
    // Both sources the register: `sbb r, 0` reads it, and its immediate is
    // not among `uses`.
    (what.name.as_deref() == Some("sbb")
        && one.uses.iter().all(|used| *used == value)
        && what.sources.len() == 2
        && what.sources.iter().all(|source| matches!(source, Loc::Held(held) if held.value == value)))
    .then_some(value)
}

/// `body` with each constant that only `sbb r, r` reads an `implicit_def`.
fn ignored_inputs_undefined(body: LirBody) -> LirBody {
    let ignored: HashSet<u32> =
        body.blocks.iter().flat_map(|block| block.insns.iter()).filter_map(|one| ignores_its_input(one)).collect();
    if ignored.is_empty() {
        return body;
    }
    // A value anything else reads stays a constant.
    let read_otherwise: HashSet<u32> = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| ignores_its_input(one).is_none())
        .flat_map(|one| {
            one.uses
                .iter()
                .chain(&one.requires.iter().map(|(held, _)| held.value).collect::<Vec<_>>())
                .copied()
                .collect::<Vec<_>>()
        })
        .chain(
            body.blocks
                .iter()
                .flat_map(|block| block.phis.iter())
                .flat_map(|phi| phi.incoming.iter().map(|&(_, value)| value)),
        )
        .collect();
    let constant = |one: &Insn| {
        one.what
            .as_ref()
            .is_some_and(
                |what| what.op == Operation::Move && what.sources.len() == 1 && matches!(what.sources[0], Loc::Imm(_)),
            )
    };
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block
                .insns
                .iter()
                .map(|one| match one.defines.as_slice() {
                    [value] if constant(one) && ignored.contains(value) && !read_otherwise.contains(value) => {
                        made_implicit(one, *value)
                    }
                    _ => Arc::clone(one),
                })
                .collect();
            block.with_insns(insns)
        })
        .collect();
    body.with_blocks(blocks)
}

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
        let body = if self.ignored_inputs { ignored_inputs_undefined(body) } else { body };
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
        // A phi that reads nothing but implicit defs is one itself
        // (`canTurnIntoImplicitDef`), at the head of the block's code.
        if current.blocks.iter().any(|block| block.phis.iter().any(|phi| all_undefined(phi, &defined))) {
            let blocks: Vec<_> = current
                .blocks
                .iter()
                .map(|block| {
                    let (undef, kept): (Vec<_>, Vec<_>) =
                        block.phis.iter().cloned().partition(|phi| all_undefined(phi, &defined));
                    if undef.is_empty() {
                        return block.clone();
                    }
                    let made: Vec<Arc<Insn>> = undef
                        .iter()
                        .map(|phi| {
                            let source = current
                                .blocks
                                .iter()
                                .flat_map(|block| block.insns.iter())
                                .find(|one| implicit_def_of(one) == Some(phi.incoming[0].1))
                                .expect("a defined value has its definition");
                            made_defining(source, phi.result)
                        })
                        .chain(block.insns.iter().cloned())
                        .collect();
                    let mut block = block.with_insns(made);
                    block.phis = kept.into();
                    block
                })
                .collect();
            current = current.with_blocks(blocks);
            continue;
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
                    .filter_map(|one| {
                        let flagged: Vec<u32> = one.uses.iter().copied().filter(|v| defined.contains(v)).collect();
                        if flagged.is_empty() {
                            return Some(Arc::clone(one));
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
                            return Some(made_implicit(&made, dst.value));
                        }
                        // A spill slot written from nothing holds nothing to
                        // reload: the pin's spiller makes the defining
                        // `IMPLICIT_DEF` again where it is read.
                        if made.uses.is_empty() && made.defines.is_empty() && stores_to_frame(&made) {
                            return None;
                        }
                        Some(Arc::new(made))
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

/// A plain store of a register to a frame cell.
fn stores_to_frame(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.op == Operation::Move
                && matches!(
                    what.dests.as_slice(),
                    [Loc::Mem(cell)] if cell.addr.is_some_and(|addr| addr.space == crate::model::ir::Space::Frame)
                ),
        )
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
        body.iter_insns().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    fn zero(value: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![held(value)],
            sources: vec![Loc::Imm(crate::model::ir::Imm { value: 0, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![value], Vec::new()))
    }

    fn sbb(
        into: u32,
        from: u32,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("sbb".into()),
            dests: vec![held(into)],
            sources: vec![held(from), held(from)],
            ..Semantics::new(Operation::Binary)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![into], vec![from, from]))
    }

    /// `sbb r, 0`: the borrow from the register it reads, not the borrow alone.
    fn sbb_zero(
        into: u32,
        from: u32,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("sbb".into()),
            dests: vec![held(into)],
            sources: vec![held(from), Loc::Imm(crate::model::ir::Imm { value: 0, width: 4, address: None })],
            ..Semantics::new(Operation::Binary)
        };
        Arc::new(Insn::new(0, None, Some(what), vec![into], vec![from]))
    }

    fn pass_over(
        ignored_inputs: bool,
        insns: Vec<Arc<Insn>>,
    ) -> LirBody {
        ProcessImplicitDefs { ignored_inputs }.transform(body(insns, Vec::new())).unwrap()
    }

    /// `mask` (`a == 0 ? -1 : 0`) at -Os m16 was `cmp ax, 1; sbb ecx, ecx; mov
    /// ax, cx`: the zero `sbb r, r` ignores was live across the compare, so the
    /// result could not take the register the argument was in. On an in-order
    /// core the zero goes and `sbb` reads undef.
    #[test]
    fn test_the_zero_an_sbb_of_one_register_ignores_is_read_undef_on_an_in_order_core() {
        let got = pass_over(true, vec![zero(1), sbb(2, 1)]);
        assert_eq!(names(&got), ["sbb"]);
        let sbb = &got.insns()[0];
        assert!(sbb.uses.is_empty() && !sbb.undefs.is_empty() && sbb.undefs.iter().all(|value| *value == 1), "{sbb:?}");
    }

    /// `(long long) a - (long long) b` at m16 was `sub eax, edx; sbb edx, 0`:
    /// the zero `sbb v1, 0` reads as the high word was dropped as one `sbb
    /// r, r` ignores, and `edx` (b) took its place: the result's high word
    /// was `b - borrow`.
    #[test]
    fn test_the_zero_an_sbb_with_an_immediate_reads_is_kept() {
        assert_eq!(names(&pass_over(true, vec![zero(1), sbb_zero(2, 1)])), ["mov", "sbb"]);
    }

    /// A core that waits for the old register keeps the zero that breaks the
    /// wait, and a zero something else reads stays a constant.
    #[test]
    fn test_the_zero_stays_where_it_is_waited_for_or_read_otherwise() {
        assert_eq!(names(&pass_over(false, vec![zero(1), sbb(2, 1)])), ["mov", "sbb"]);
        assert_eq!(names(&pass_over(true, vec![zero(1), sbb(2, 1), add(3, 4, 1)])), ["mov", "sbb", "add"]);
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
        let phi = Phi { result: 5, incoming: vec![(0, 1), (1, 6)] };
        let got = processed(&body(vec![def(1), add(3, 4, 1)], vec![phi]));
        assert_eq!(names(&got), ["implicit_def", "add"]);
    }

    /// A phi whose every operand is an `implicit_def` is one itself, worked in
    /// turn (`canTurnIntoImplicitDef` takes a PHI): its users read undef and
    /// neither definition nor phi is left. Left a phi, PHIElimination made the
    /// implicit def after this pass ran, and the add read a live register.
    #[test]
    fn test_a_phi_of_implicit_defs_is_an_implicit_def_and_its_users_read_undef() {
        let phi = Phi { result: 5, incoming: vec![(0, 1), (1, 1)] };
        let got = processed(&body(vec![def(1), add(3, 4, 5)], vec![phi]));
        assert_eq!(names(&got), ["add"]);
        assert_eq!(got.insns()[0].undefs, vec![5]);
        assert!(got.blocks[0].phis.is_empty());
    }
}
