//! Removes a copy whose two values can share a register: the phase that runs
//! LLVM's `RegisterCoalescer` ([`crate::coalescer`]).
//!
//! # How it works
//! After `phielim` and `twoaddr` the body is full of `mov v2, v1` between
//! values. If the live intervals of `v1` and `v2` can be joined (the values
//! they hold agree wherever both are live), they are one value and the move
//! is an identity that disappears:
//!
//! ```text
//!  before                          after
//!    v1 = load ...                   v1 = load ...
//!    v2 = mov v1       (no clash)    ... v1 ...     v2 renamed v1
//!    ... v2 ...                      the mov is gone
//! ```
//!
//! The phase builds the intervals ([`crate::liveness::liveintervals`]) and
//! hands them to [`crate::coalescer`] with the target's answers (which values
//! may share a register: the palettes and the pins). Joining is by interval
//! and leaves colouring to the allocator, which splits where it must; the old
//! pass here (Briggs and George over an interference graph, with a call-clobber
//! check) is gone, objects unchanged on the corpora the day it went. A body
//! with phis is not coalesced.
//!
//! # Example
//! A multiply by 5: `v3` joins `v4`, so `v3 := mov v4` goes and the add updates
//! `v4`. `v5 := mov v4` stays: `v4` and `v5` are pinned to EAX and EDX. `;
//! inserted` notes are dropped.
//!
//! Before (`TwoAddress`):
//!
//! ```lir
//! ; ...
//! 0x0001  v4 := mov v2
//! 0x0001  v4 := shl v4, 0x2
//! 0x0001  v3 := mov v4
//! 0x0001  v3 := add v3, v2
//! 0x0002  v5 := mov v3
//! 0x0002  v5 := shr v5, 0x10
//! 0x0002     ; requires v3@EAX,v5@EDX
//! ```
//!
//! After (`Coalescer`):
//!
//! ```lir
//! ; ...
//! 0x0001  v4 := mov v2
//! 0x0001  v4 := shl v4, 0x2
//! 0x0001  v4 := add v4, v2
//! 0x0002  v5 := mov v4
//! 0x0002  v5 := shr v5, 0x10
//! 0x0002     ; requires v4@EAX,v5@EDX
//! ```
//!
//! From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of `Times5& = x * 5` (function
//! `TIMES5&`).
//!
//! # Runs
//! Once per function, in the machine phases after `twoaddr` and before
//! `regalloc`, at every -O level. Both routes run it.
//!
//! # See also
//! LLVM's `RegisterCoalescer`:
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp>

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

use crate::analysis::intervals as ranges;
use crate::backend::classes::RegisterClasses;
use crate::backend::regclass;
use crate::backend::target::{self, Segments};
use crate::model::lir::LirBody;
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

pub struct Coalescer {
    pub pinned: IndexMap<u32, RegId>,
    pub segments: Segments,
    pub classes: std::rc::Rc<RegisterClasses>,
    /// A build for size.
    pub size: bool,
}

impl Coalescer {
    pub const NAME: &'static str = "coalesce";

    pub fn new(
        pinned: Option<&IndexMap<u32, RegId>>,
        segments: &Segments,
        classes: &std::rc::Rc<RegisterClasses>,
    ) -> Self {
        Self {
            pinned: pinned.cloned().unwrap_or_default(),
            segments: segments.clone(),
            classes: std::rc::Rc::clone(classes),
            size: false,
        }
    }
}

impl LIRTransform for Coalescer {
    fn class_name(&self) -> &'static str {
        "Coalescer"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(match self.llvm_env(&body) {
            Some((env, depth)) => crate::coalescer::env::joined(&body, &env, &depth),
            // A body with phis is not translated: phi elimination runs first.
            None => body,
        })
    }
}

impl Coalescer {
    /// What the translated coalescer asks of the target for `body`: the
    /// registers each value may take and the ones that are fixed. None where
    /// it does not apply (a body with phis).
    fn llvm_env(
        &self,
        body: &LirBody,
    ) -> Option<(crate::coalescer::env::TargetEnv, Vec<u32>)> {
        if body.blocks.iter().any(|block| !block.phis.is_empty()) {
            return None;
        }
        let mut pins = body.pins.clone();
        pins.extend(self.pinned.iter().map(|(value, register)| (*value, *register)));
        let pins: IndexMap<u32, RegId> =
            pins.into_iter().map(|(value, register)| (value, body.regs().root(register))).collect();
        let mut where_of = regclass::classes(body, &BTreeSet::new(), &self.segments, &self.classes);
        // A value made in another's class (a phi's temporary, in its result's)
        // has that one where its own uses give it none.
        for (value, like) in body.class_like.iter() {
            if !where_of.contains_key(value)
                && let Some(class) = where_of.get(like).cloned()
            {
                where_of.insert(*value, class);
            }
        }
        let everything: BTreeSet<RegId> = self.classes.available.iter().copied().collect();
        let mut palettes: IndexMap<u32, BTreeSet<RegId>> = IndexMap::default();
        for block in &body.blocks {
            for one in block.insns.iter() {
                for value in one.defines.iter().chain(&one.uses) {
                    palettes
                        .entry(*value)
                        .or_insert_with(
                            || target::order(where_of.get(value), &self.segments, &self.classes).into_iter().collect(),
                        );
                }
            }
        }
        for (value, register) in &pins {
            if !everything.contains(register) && !where_of.contains_key(value) {
                palettes.insert(*value, BTreeSet::from([*register]));
            }
        }
        let depths = ranges::depths_shared(body);
        let depth = body.blocks.iter().map(|block| depths.get(&block.at).copied().unwrap_or(0)).collect();
        Some((crate::coalescer::env::TargetEnv { palettes: palettes.into(), pins, size: self.size }, depth))
    }
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_coalesce.py`.

    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use crate::backend::{allocresult, select};
    use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Semantics {
        Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
    }

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn _move(
        at: i64,
        into: u32,
        out_of: u32,
    ) -> Insn {
        let what = semantics(Operation::Move, "mov", vec![held(into, 2)], vec![held(out_of, 2)]);
        Insn::new(at, Some((at, at + 2)), Some(what), vec![into], vec![out_of])
    }

    fn _use(
        at: i64,
        reads: u32,
    ) -> Insn {
        let what = semantics(Operation::Push, "push", vec![], vec![held(reads, 2)]);
        Insn::new(at, Some((at, at + 1)), Some(what), vec![], vec![reads])
    }

    fn _define(
        at: i64,
        makes: u32,
    ) -> Insn {
        let what = semantics(
            Operation::Move,
            "mov",
            vec![held(makes, 2)],
            vec![Loc::Imm(Imm { value: i64::from(makes), width: 2, address: None })],
        );
        Insn::new(at, Some((at, at + 3)), Some(what), vec![makes], vec![])
    }

    fn _jump(
        at: i64,
        to: i64,
    ) -> Insn {
        let what = Semantics { name: Some("jmp".into()), target: Some(to), ..Semantics::new(Operation::Jump) };
        Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![])
    }

    fn body(
        name: &str,
        insns: Vec<Insn>,
        pins: &[(u32, RegId)],
    ) -> LirBody {
        LirBody::new(
            name,
            0,
            vec![LirBlock::new(0, insns.into_iter().map(Arc::new).collect()).into()],
            IndexMap::default(),
            pins.iter().copied().collect(),
        )
    }

    /// The assignment the pins give: nothing here needs an allocator.
    fn allocated(
        _body: &LirBody,
        pins: &IndexMap<u32, RegId>,
    ) -> allocresult::Assignment {
        let mut values = pins.clone();
        values.insert(3, RegId::ES);
        allocresult::Assignment {
            r#where: values,
            spilled: Default::default(),
            cost: 0.0,
            optimal: false,
            why: "pins".to_owned(),
        }
    }

    /// HARR's hoisted selector copy became unencodable mov es,es across a
    /// coverage gap.
    #[test]
    fn test_retained_resource_identity_has_a_legal_encoding() {
        let body = body("resource-copy", vec![_move(3, 1, 1)], &[(1, RegId::ES)]);
        let result = crate::backend::rewriter::rewrite(
            &body,
            &allocated(&body, &body.pins),
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("applies");
        let insns = result.insns();
        assert_eq!(insns.len(), 1);
        assert_eq!(insns[0].covers, body.insns()[0].covers);
        let emitted = select::emit(insns[0].what.as_ref().expect("semantics"), 3, None, false, false, None);
        assert!(emitted.is_some());
        assert!(emitted.unwrap().code.is_empty());
    }
}

#[cfg(test)]
mod handoff_tests {
    use crate::backend::regalloc_input::{Calls, before_phase};
    use crate::liveness::liveintervals::{BUILT, take_handed_over};

    /// TwoAddress reads `LiveVariables` and leaves no intervals (the pin's
    /// `LiveIntervals` is one analysis built before the coalescer, which is
    /// `intervals-once.md` step 3): until then the coalescer builds its own,
    /// once per function.
    #[test]
    fn test_the_coalescer_builds_its_intervals_once_when_two_address_leaves_none() {
        let (body, mut phases) = before_phase(Calls::Everything, "sievejumps.ll", "bench_sieve", "486", "Coalescer");
        assert!(take_handed_over(&body).is_none(), "the premise: TwoAddress left no intervals");
        let before = BUILT.with(std::cell::Cell::get);
        phases[0].transform(body).expect("coalesces");
        assert_eq!(BUILT.with(std::cell::Cell::get) - before, 1);
    }
}
