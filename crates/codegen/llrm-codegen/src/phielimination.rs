//! `PHIElimination`: phis become copies, each phi through a register of its
//! own. Translated from llvm/lib/CodeGen/PHIElimination.cpp at d1106deb71cc
//! (`SplitPHIEdges`, `EliminatePHINodes`, `LowerPHINode`, `isLiveIn`,
//! `isLiveOutPastPHIs`).
//!
//! # How it works
//! A phi `d = phi [p1: a, p2: b]` becomes `i = copy a` at the end of `p1`
//! (before its terminators), `i = copy b` at the end of `p2`, and `d = copy i`
//! at the top of the phi's block, where `i` is a register made for this phi.
//! Nothing reads `i` but that copy, so the copies on one edge cannot disturb
//! one another (no parallel copy, no lost copy, no swap problem) and the pass
//! needs no price: the coalescer joins `i` with `a`, `b` and `d` where their
//! ranges allow.
//!
//! Before that, critical edges are split where the copy would otherwise stay
//! (`SplitPHIEdges`): an edge `p -> s` whose value is live out of `p` past the
//! phi and is not live into `s` (the interference is then avoidable by a block
//! on the edge), or that leaves a loop, unless it is a back edge or enters the
//! loop from outside. A block whose predecessors all have several successors
//! lowers identical phis through one register.
//!
//! No deviation for a phi that reads from a block outside the body: LLVM's phis
//! list only predecessors in their function, so it is an error here too (the
//! verifier says the same, `a phi ... is not a predecessor`); it was left alone
//! before.
//!
//! DEVIATE P6 (exit: a class set at creation, once isel allocates classes,
//! CF-3 in docs/working/vreg-class-field-plan.md; owner mir-undef; review
//! 2026-11-14): a temporary is made in its result's class by name
//! (`LirBody::class_like`), where the pin's `createVirtualRegister` gives it
//! the class the result has, which the type gave the result when isel made it.
//! There is no per-value class at isel time here, and computing the result's
//! class at this pass cost +0.40% Ir on d_faces -O2 (measured, #3625); so the
//! class is what `regclass::classes` finds from the result's uses when a pass
//! asks (`Coalescer::llvm_env`). A far pointer's selector read through ES is
//! the case (render/dl).
//!
//! DEVIATE P7 (no number): `findPHICopyInsertPoint` and `LowerPHINode` do not
//! treat an EH-pad successor or an `INLINEASM_BR` indirect target. The unwind
//! edge of an `invoke` is the runtime's, not an edge of the LIR body (the
//! selector's successor list keeps the normal one, `isel.rs`), so no phi copy
//! is asked of it and a landing pad has no machine predecessor to copy in; and
//! no front end emits `callbr`. A front end that does has to restore both where
//! each edge's copies are placed.
//!
//! DEVIATE P3 (no number): `MachineLoopInfo::getLoopFor` is the innermost loop
//! of the body's loop forest; `isLiveIn` and `isLiveOutPastPHIs` read the
//! values live at a block's entry (`liverows::live`), where
//! `LiveIntervals::liveAt` reads the interval. A phi's operand is read at the
//! end of its predecessor, so it is not live into the successor by that phi, as
//! in `LiveIntervals`.
//!
//! # Example
//! A loop carrying `i`: `i2 = phi [pre: i0, body: i1]` becomes `n = copy i0`
//! at the end of `pre`, `n = copy i1` at the end of `body` and `i2 = copy n` at
//! the top of the loop.
//!
//! # Used by
//! `backend::phielim::PhiElimination`.

use std::sync::Arc;

use crate::backend::phielim::{self, Split};
use crate::model::ir::Held;
use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};
use crate::support::hash::{HashMap, IndexMap};

#[cfg(test)]
thread_local! {
    /// The liveness rows `edges_to_split` solved, for the test that fixes it at
    /// none where no critical edge asks.
    static ROWS_SOLVED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

/// `SplitPHIEdges`: the critical edges worth a block of their own, as
/// `(predecessor, block)`.
fn edges_to_split(
    body: &LirBody,
    at_of: &IndexMap<i64, &Block>,
) -> Vec<(i64, i64)> {
    // Asked one value and block at a time: the rows answer it, without a set
    // for every block's entry and exit (`LiveIntervals::liveAt`). Solved when
    // the first critical edge asks (the pin reads the `LiveVariables` it
    // already has): most bodies have none.
    let rows = std::cell::OnceCell::new();
    let forest = crate::analysis::loops::forest_of(&body.blocks, Some(body.entry));
    // Only the values a critical edge's phi hands over are asked: rows of
    // those.
    let asked: llrm_support::hash::HashSet<u32> = body
        .blocks
        .iter()
        .flat_map(|block| block.phis.iter())
        .flat_map(|phi| phi.incoming.iter())
        .filter(|(pre, _)| at_of.get(pre).is_some_and(|block| block.succ.len() > 1))
        .map(|&(_, reg)| reg)
        .collect();
    let live_in = |value: u32, at: i64| {
        let rows = rows.get_or_init(|| {
            #[cfg(test)]
            ROWS_SOLVED.with(|one| one.set((one.get().0 + 1, one.get().1 + asked.len())));
            crate::backend::liverows::live_rows_by(body, |one| asked.contains(&one))
        });
        crate::backend::liverows::LiveAt::live_in(rows, at, value)
    };
    let mut split: Vec<(i64, i64)> = Vec::new();
    for block in &body.blocks {
        if block.phis.is_empty() {
            continue;
        }
        let cur_loop = forest.innermost(block.at);
        let is_loop_header = cur_loop.is_some_and(|one| forest.loops[one].header == block.at);
        for phi in block.phis.iter() {
            for &(pre, reg) in &phi.incoming {
                let Some(pre_block) = at_of.get(&pre) else { continue };
                // Is there a critical edge from PreMBB to MBB?
                if pre_block.succ.len() == 1 {
                    continue;
                }
                // Avoid splitting backedges of loops. It would introduce small
                // out-of-line blocks into the loop which is very bad for code
                // placement.
                if pre == block.at {
                    continue;
                }
                let pre_loop = forest.innermost(pre);
                if is_loop_header && pre_loop == cur_loop {
                    continue;
                }
                // The copy in PreMBB is not a kill when the value is live out
                // for some other reason than a phi use, and may not coalesce.
                let mut should_split = pre_block.succ.iter().any(|&succ| live_in(reg, succ));
                if !should_split {
                    continue;
                }
                // If Reg is not live-in to MBB, it must be live-in to some
                // other PreMBB successor, and splitting avoids the
                // interference. If it is, the interference is inevitable.
                should_split = !live_in(reg, block.at);
                // Check for a loop exiting edge.
                if !should_split && cur_loop != pre_loop {
                    // This edge could be entering a loop, exiting a loop, or
                    // both. Split unless it is entering CurLoop from an outer
                    // loop.
                    should_split = pre_loop.is_some_and(|outer| {
                        !cur_loop.is_some_and(|inner| forest.contains(outer, forest.loops[inner].header))
                    });
                }
                if should_split && !split.contains(&(pre, block.at)) {
                    split.push((pre, block.at));
                }
            }
        }
    }
    split
}

/// `body` with every phi lowered to copies.
pub fn lowered(body: &LirBody) -> Result<LirBody, String> {
    if body.blocks.iter().all(|block| block.phis.is_empty()) {
        return Ok(body.clone());
    }
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            if let Some((pre, _)) = phi.incoming.iter().find(|(pre, _)| !at_of.contains_key(pre)) {
                return Err(format!(
                    "{}: the phi at {:#06x} reads from {pre:#06x}, which is not a block of the body",
                    body.name, block.at
                ));
            }
        }
    }
    let widths = phielim::_widths(body);
    let kept_phis = |body: &LirBody| -> IndexMap<i64, Vec<Phi>> {
        body.blocks.iter().map(|block| (block.at, block.phis.to_vec())).collect()
    };
    let split = edges_to_split(body, &at_of);
    let split_body;
    let body = if split.is_empty() {
        body
    } else {
        let edges: IndexMap<(i64, i64), Split> = split.iter().map(|&edge| (edge, Split::Pairs(Vec::new()))).collect();
        split_body = phielim::_split_edges(
            body,
            &edges,
            &IndexMap::default(),
            &IndexMap::default(),
            &kept_phis(body),
            &widths,
            false,
            &|_, _| false,
        )?;
        &split_body
    };
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut next = body.next_value();
    // `isImplicitlyDefined`: every definition is an `implicit_def`.
    let undefined: crate::support::hash::HashSet<u32> = {
        let mut seen: HashMap<u32, (usize, usize)> = HashMap::default();
        for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
            let implicit = crate::processimpdefs::implicit_def_of(one).is_some();
            for &value in &one.defines {
                let entry = seen.entry(value).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += usize::from(implicit);
            }
        }
        seen.into_iter().filter(|&(_, (all, implicit))| all == implicit).map(|(value, _)| value).collect()
    };
    // `ImpDefs`: the `implicit_def`s a phi read, erased once nothing reads
    // them.
    let mut imp_defs: crate::support::hash::HashSet<u32> = crate::support::hash::HashSet::default();
    let mut head: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut tails: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut born: IndexMap<u32, u32> = IndexMap::default();
    let mut preds: IndexMap<i64, usize> = IndexMap::default();
    for block in &body.blocks {
        for &to in block.succ.iter() {
            *preds.entry(to).or_insert(0) += 1;
        }
    }
    // `LoweredPHIs`: kept for the whole function, so identical phis of
    // different blocks share the temporary.
    let mut lowered_phis: HashMap<Vec<(i64, u32)>, u32> = HashMap::default();
    for block in &body.blocks {
        // If all incoming edges are critical, identical phis are lowered
        // through one register, so fewer copies are made.
        let all_critical = preds.get(&block.at).copied().unwrap_or(0) >= 2
            && body.blocks.iter().filter(|one| one.succ.contains(&block.at)).all(|one| one.succ.len() >= 2);
        for phi in block.phis.iter() {
            let width = widths[&phi.result];
            // `allPhiOperandsUndefined`: an `implicit_def` of the result, and
            // no copy on any edge.
            if phi.incoming.iter().all(|(_, value)| undefined.contains(value)) {
                head.entry(block.at).or_default().push(_implicit_def(block, phi.result, width));
                continue;
            }
            let key = {
                let mut key = phi.incoming.clone();
                key.sort_unstable();
                key
            };
            let reused = if all_critical { lowered_phis.get(&key).copied() } else { None };
            let incoming = reused.unwrap_or_else(|| {
                let made = next;
                next += 1;
                made
            });
            if all_critical && reused.is_none() {
                lowered_phis.insert(key, incoming);
            }
            if reused.is_none() {
                born.insert(incoming, phi.result);
            }
            // The copy at the top of the block, after any remaining phis
            // (`head` is in lowering order; the block gets it
            // reversed).
            head.entry(block.at).or_default().push(phielim::_copy(block, phi.result, incoming, None, width));
            if reused.is_some() {
                continue;
            }
            // The copy at the end of each predecessor, once for each.
            let mut done: Vec<i64> = Vec::new();
            for &(pre, source) in phi.incoming.iter().rev() {
                if done.contains(&pre) {
                    continue;
                }
                done.push(pre);
                // `SrcUndef`: nothing to copy, but a definition to keep the
                // temporary jointly dominated.
                let made = if undefined.contains(&source) {
                    imp_defs.insert(source);
                    _implicit_def(at_of[&pre], incoming, width)
                } else {
                    phielim::_copy(at_of[&pre], incoming, source, None, width)
                };
                tails.entry(pre).or_default().push(made);
            }
        }
    }
    let mut out = body.clone();
    if !born.is_empty() {
        Arc::make_mut(&mut out.class_like).extend(born);
    }
    for block in &mut out.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        let first = block.insns.iter().position(|one| !one.arrival()).unwrap_or(block.insns.len());
        let tail = phielim::_before_the_terminator(block, tails.get(&block.at).map_or(&[][..], Vec::as_slice));
        insns.extend(tail[..first].iter().cloned());
        // Each copy is made before the ones made earlier: the pin inserts at
        // `std::next(LastPHIIt)`, the node after the last phi, which is the
        // previous copy once there is one.
        insns.extend(head.get(&block.at).into_iter().flatten().rev().cloned());
        insns.extend(tail[first..].iter().cloned());
        *block = LirBlock { insns: insns.into(), phis: Vec::new().into(), ..(**block).clone() }.into();
    }
    if !imp_defs.is_empty() {
        // Remove dead `implicit_def`s: the definitions no phi's source copy
        // reads now.
        let read: crate::support::hash::HashSet<u32> =
            out.blocks.iter().flat_map(|block| block.insns.iter()).flat_map(|one| one.uses.iter().copied()).collect();
        for block in &mut out.blocks {
            if block.insns.iter().any(|one| {
                crate::processimpdefs::implicit_def_of(one)
                    .is_some_and(|value| imp_defs.contains(&value) && !read.contains(&value))
            }) {
                let insns: Vec<Arc<Insn>> = block
                    .insns
                    .iter()
                    .filter(|one| {
                        !crate::processimpdefs::implicit_def_of(one)
                            .is_some_and(|value| imp_defs.contains(&value) && !read.contains(&value))
                    })
                    .cloned()
                    .collect();
                *block = LirBlock { insns: insns.into(), ..(**block).clone() }.into();
            }
        }
    }
    Ok(out)
}

/// An `implicit_def` of `value`, placed like a copy into `block` (`_copy`).
fn _implicit_def(
    block: &LirBlock,
    value: u32,
    width: u32,
) -> Arc<Insn> {
    let last = block.insns.last();
    let at = last.map_or(block.at, |last| last.at);
    let edge = phielim::_nothing(last.map(|last| &**last), at);
    let what = crate::model::ir::Semantics {
        op: crate::model::ir::Operation::Nothing,
        name: Some(crate::processimpdefs::IMPLICIT_DEF.into()),
        dests: vec![crate::model::ir::Loc::Held(Held { value, width })],
        sources: Vec::new(),
        ..crate::model::ir::Semantics::new(crate::model::ir::Operation::Nothing)
    };
    let mut one = Insn::new(at, Some(edge), Some(what), vec![value], Vec::new());
    one.call = last.and_then(|last| last.call.clone());
    Arc::new(one)
}

#[allow(dead_code)]
fn _held(
    value: u32,
    width: u32,
) -> Held {
    Held { value, width }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Imm, Loc, Operation, Semantics};

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn def(v: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![held(v)],
            sources: vec![Loc::Imm(Imm { value: 7, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![v], vec![]))
    }

    fn read(v: u32) -> Arc<Insn> {
        Arc::new(Insn::new(0, Some((0, 0)), None, vec![], vec![v]))
    }

    fn jump(target: i64) -> Arc<Insn> {
        let what = Semantics { name: Some("jmp".into()), target: Some(target), ..Semantics::new(Operation::Jump) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn branch(target: i64) -> Arc<Insn> {
        let what = Semantics { name: Some("jne".into()), target: Some(target), ..Semantics::new(Operation::Branch) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: &[i64],
        phis: Vec<Phi>,
    ) -> LirBlock {
        LirBlock { succ: succ.to_vec().into(), phis: phis.into(), ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new(
            "lowered",
            0,
            blocks.into_iter().map(Block::from).collect(),
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    fn copies(block: &Block) -> Vec<(u32, u32)> {
        block.insns.iter().filter_map(|one| one.copy_of()).collect()
    }

    fn undef(v: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some(crate::processimpdefs::IMPLICIT_DEF.into()),
            dests: vec![held(v)],
            ..Semantics::new(Operation::Nothing)
        };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![v], vec![]))
    }

    fn implicit_defs(block: &Block) -> Vec<u32> {
        block.insns.iter().filter_map(|one| crate::processimpdefs::implicit_def_of(one)).collect()
    }

    /// A phi of values only `implicit_def`s define was lowered like any other:
    /// a copy of each into the temporary on every edge, then a copy out. The
    /// pin makes it one `implicit_def` of the result and no copy
    /// (`allPhiOperandsUndefined`).
    #[test]
    fn test_a_phi_of_undefined_values_is_an_implicit_def_of_its_result_and_no_copy() {
        let phis = vec![Phi { result: 10, incoming: vec![(0, 1), (1, 2)] }];
        let was = body(vec![
            block(0, vec![undef(1), branch(2)], &[1, 2], vec![]),
            block(1, vec![undef(2), jump(2)], &[2], vec![]),
            block(2, vec![read(10)], &[], phis),
        ]);
        let got = lowered(&was).expect("lowers");
        assert!(got.blocks.iter().all(|block| copies(block).is_empty()), "no copy on any edge");
        assert_eq!(implicit_defs(&got.blocks[2]), [10], "the result is undefined where the phi was");
    }

    /// An incoming value only an `implicit_def` defines was copied into the
    /// temporary on its edge, and its definition stayed: the pin makes it an
    /// `implicit_def` of the temporary there (`SrcUndef`) and erases the old
    /// one once nothing reads it (`ImpDefs`).
    #[test]
    fn test_an_undefined_incoming_value_is_an_implicit_def_of_the_temporary_on_its_edge() {
        let phis = vec![Phi { result: 10, incoming: vec![(0, 1), (1, 2)] }];
        let was = body(vec![
            block(0, vec![undef(1), branch(2)], &[1, 2], vec![]),
            block(1, vec![def(2), jump(2)], &[2], vec![]),
            block(2, vec![read(10)], &[], phis),
        ]);
        let got = lowered(&was).expect("lowers");
        let temporary = copies(&got.blocks[2])[0].1;
        assert_eq!(implicit_defs(&got.blocks[0]), [temporary], "the undefined edge defines the temporary");
        assert!(copies(&got.blocks[0]).is_empty(), "and copies nothing");
        assert_eq!(copies(&got.blocks[1]), [(temporary, 2)], "the defined edge still copies");
        assert!(!implicit_defs(&got.blocks[0]).contains(&1), "the old definition went");
    }

    /// `SplitPHIEdges` asks whether one value is live into one block, and was
    /// answered by the sets of every block's entry and exit (two per body,
    /// 14 M of d_faces -O2's 19.6 M in `liverows::live` over the pass's three
    /// calls); the rows answer it with no set made.
    #[test]
    fn test_lowering_builds_no_block_sets() {
        use crate::backend::liverows::SETS_BUILT;
        let phis = vec![Phi { result: 10, incoming: vec![(0, 1), (1, 12)] }];
        let was = body(vec![
            block(0, vec![def(1), jump(1)], &[1], vec![]),
            block(1, vec![def(12), read(10), branch(1)], &[1, 2], phis),
            block(2, vec![read(10)], &[], vec![]),
        ]);
        SETS_BUILT.with(|one| one.set(0));
        lowered(&was).expect("lowers");
        let built = SETS_BUILT.with(|one| one.get());
        assert_eq!(built, 0, "{built} block sets built to lower one body");
    }

    /// A phi of an 8-byte integer (a `long long` in m32) is lowered like any
    /// other. It was left for `phielim`'s own priced lowering, where the pin's
    /// legalizer has made it two 4-byte phis before PHIElimination runs
    /// (d_faces -O2: that stage cost 11.7 M instructions on one body).
    #[test]
    fn test_a_phi_of_an_8_byte_integer_is_lowered_like_any_other() {
        let wide = |value: u32| Loc::Held(Held { value, width: 8 });
        let make = |value: u32| {
            let what = Semantics {
                name: Some("mov".into()),
                dests: vec![wide(value)],
                sources: vec![Loc::Imm(Imm { value: 7, width: 8, address: None })],
                ..Semantics::new(Operation::Move)
            };
            Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![value], vec![]))
        };
        let phis = vec![Phi { result: 10, incoming: vec![(0, 1), (1, 12)] }];
        let was = body(vec![
            block(0, vec![make(1), jump(1)], &[1], vec![]),
            block(1, vec![make(12), read(10), branch(1)], &[1, 2], phis),
            block(2, vec![read(10)], &[], vec![]),
        ]);
        let now = lowered(&was).expect("lowers");
        assert_eq!(now.blocks.iter().map(|one| one.phis.len()).sum::<usize>(), 0, "no phi is left for the old stage");
    }

    /// The x87 phi of a loop is lowered like any other (the old route left it
    /// for `phielim`'s own lowering, which is deleted).
    #[test]
    fn test_an_x87_phi_is_lowered_like_any_other() {
        let float = |value: u32| Loc::Held(Held { value, width: 10 });
        let load = |value: u32| {
            let what = Semantics {
                name: Some("fldz".into()),
                dests: vec![float(value)],
                ..Semantics::new(Operation::FloatLoad)
            };
            Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![value], vec![]))
        };
        let store = |value: u32| {
            let what = Semantics {
                name: Some("fstp".into()),
                dests: vec![Loc::Reg(crate::model::ir::Reg { register: llrm_lir::registers::RegId::BX, width: 4 })],
                sources: vec![float(value)],
                ..Semantics::new(Operation::FloatStore)
            };
            Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![value]))
        };
        let phis = vec![Phi { result: 10, incoming: vec![(0, 1), (1, 12)] }];
        let was = body(vec![
            block(0, vec![load(1), jump(1)], &[1], vec![]),
            block(1, vec![load(12), store(10), branch(1)], &[1, 2], phis),
            block(2, vec![store(10)], &[], vec![]),
        ]);
        let phis_left = |now: &LirBody| now.blocks.iter().map(|one| one.phis.len()).sum::<usize>();
        assert_eq!(phis_left(&lowered(&was).expect("lowers")), 0, "lowered");
    }

    /// Two counters carried round a loop: each phi gets a register of its own,
    /// the copies at the end of the loop write those, and the top of the loop
    /// copies them into the phis' results, the last phi's first (the pin's
    /// `LowerPHINode` inserts each copy before the earlier ones).
    ///
    /// x_list's sum loop kept the copy of the new pointer ahead of the copy of
    /// the old one, so the old pointer was live across the new one's definition
    /// and took a register of its own: +1 instruction a trip.
    #[test]
    fn test_independent_phis_are_lowered_through_registers_of_their_own() {
        let phis = vec![
            Phi { result: 10, incoming: vec![(0, 1), (1, 12)] },
            Phi { result: 11, incoming: vec![(0, 2), (1, 13)] },
        ];
        let was = body(vec![
            block(0, vec![def(1), def(2), jump(1)], &[1], vec![]),
            block(1, vec![def(12), def(13), read(10), read(11), branch(1)], &[1, 2], phis),
            block(2, vec![read(10)], &[], vec![]),
        ]);
        let now = lowered(&was).expect("lowers");
        assert!(now.blocks.iter().all(|one| one.phis.is_empty()), "no phi is left");
        let (pre, lop) = (copies(&now.blocks[0]), copies(&now.blocks[1]));
        assert_eq!(pre.len(), 2, "one copy for each phi at the end of the entry: {pre:?}");
        assert_eq!(lop.len(), 4, "{lop:?}");
        assert_eq!((lop[0].0, lop[1].0), (11, 10), "the results are copied at the top, last phi first");
        let (first, second) = (lop[2], lop[3]);
        assert_eq!((first.1, second.1), (12, 13));
        assert!(first.0 != 10 && first.0 != 11 && second.0 != 10 && second.0 != 11, "into registers made for the phis");
        assert_eq!((lop[0].1, lop[1].1), (second.0, first.0), "the top reads what the end wrote");
    }

    /// `a = phi [.., b]`, `b = phi [.., c]`: a hands over `b`, which another
    /// phi of the block rewrites. Each phi is lowered through a register of
    /// its own, as LLVM's are; left to `phielim`'s parallel copy (DEVIATE P5)
    /// they cost idiv_loop_os 14% of its estimate at -O2 and recgcd as much.
    /// A phi that reads from a block that is not in the body: LLVM has no such
    /// phi, and the old pass left it a phi for `phielim`'s own lowering. It
    /// is an error now (no program of 71 x 2 vsgcc and 65 QCport
    /// modules has one: the verifier rule found none).
    #[test]
    fn test_a_phi_reading_from_a_block_outside_the_body_is_refused() {
        let phi = Phi { result: 5, incoming: vec![(0, 1), (9, 2)] };
        let was =
            body(vec![block(0, vec![def(1), def(2), jump(1)], &[1], vec![]), block(1, vec![read(5)], &[], vec![phi])]);
        let got = lowered(&was);
        assert!(got.as_ref().is_err_and(|why| why.contains("not a block of the body")), "{got:?}");
    }

    #[test]
    fn test_phis_that_read_each_other_are_lowered_like_any_other() {
        let phis = vec![
            Phi { result: 10, incoming: vec![(0, 1), (1, 11)] },
            Phi { result: 11, incoming: vec![(0, 2), (1, 12)] },
        ];
        let was = body(vec![
            block(0, vec![def(1), def(2), jump(1)], &[1], vec![]),
            block(1, vec![def(12), read(10), read(11), branch(1)], &[1, 2], phis),
            block(2, vec![read(10)], &[], vec![]),
        ]);
        let now = lowered(&was).expect("lowers");
        assert!(now.blocks.iter().all(|one| one.phis.is_empty()), "a phi was left");
        assert_eq!(copies(&now.blocks[0]).len(), 2, "one copy per phi on the entry edge");
    }

    /// The edge from a block with two successors to a phi's block is split
    /// when the phi's value is also live into the other successor (the copy
    /// would otherwise interfere with it): a block of only a jump is made.
    #[test]
    fn test_a_critical_edge_is_split_where_the_value_is_live_into_the_other_successor() {
        let phis = vec![Phi { result: 10, incoming: vec![(0, 1), (3, 2)] }];
        let was = body(vec![
            block(0, vec![def(1), branch(2)], &[2, 1], vec![]),
            block(1, vec![read(1), jump(3)], &[3], vec![]),
            block(2, vec![read(10), read(1)], &[], vec![]),
            block(3, vec![def(2), jump(2)], &[2], vec![]),
        ]);
        // Block 2 is entered from 0 (value 1 live out and into block 1) and
        // from 3.
        let mut was = was;
        was.blocks[2] = Block::from(block(2, vec![read(10)], &[], phis));
        let now = lowered(&was).expect("lowers");
        assert_eq!(now.blocks.len(), 5, "a block was made on the edge 0 -> 2");
        let made = now.blocks.last().expect("the new block");
        assert_eq!(made.succ.to_vec(), vec![2]);
        assert!(!now.blocks[0].succ.contains(&2), "the branch no longer goes to the phi's block");
        assert_eq!(copies(made).len(), 1, "the copy is in the edge block");
    }

    /// The rows of liveness were solved for every value of every body with a
    /// phi, though only the values a critical edge's phi hands over are asked
    /// (d_faces dist: 1.5 M Ir in three solves). Asked of a body with none they
    /// are not solved, and otherwise only for those values.
    #[test]
    fn test_the_liveness_rows_are_solved_only_where_a_critical_edge_asks() {
        let phis = vec![
            Phi { result: 10, incoming: vec![(0, 1), (1, 11)] },
            Phi { result: 11, incoming: vec![(0, 2), (1, 12)] },
        ];
        let plain = body(vec![
            block(0, vec![def(1), def(2), jump(1)], &[1], vec![]),
            block(1, vec![def(12), read(10), read(11), branch(1)], &[1, 2], phis),
            block(2, vec![read(10)], &[], vec![]),
        ]);
        let before = ROWS_SOLVED.with(std::cell::Cell::get);
        lowered(&plain).expect("lowers");
        assert_eq!(ROWS_SOLVED.with(std::cell::Cell::get), before, "no critical edge, no rows");
        let phis = vec![Phi { result: 10, incoming: vec![(0, 1), (3, 2)] }];
        let mut critical = body(vec![
            block(0, vec![def(1), branch(2)], &[2, 1], vec![]),
            block(1, vec![read(1), jump(3)], &[3], vec![]),
            block(2, vec![read(10), read(1)], &[], vec![]),
            block(3, vec![def(2), jump(2)], &[2], vec![]),
        ]);
        critical.blocks[2] = Block::from(block(2, vec![read(10)], &[], phis));
        let before = ROWS_SOLVED.with(std::cell::Cell::get);
        lowered(&critical).expect("lowers");
        let after = ROWS_SOLVED.with(std::cell::Cell::get);
        assert_eq!((after.0 - before.0, after.1 - before.1), (1, 1), "one solve, of the one value the edge hands over");
    }

    /// x_funcptr at m32: the four cases of a jump-table dispatch each took the
    /// same four phis from the dispatch and from its copy in the loop's
    /// preheader, and each phi got its own temporary and copy there (16 copies
    /// where the pin has 4, and the coalescer then joined the zero with the
    /// wrong lane). `LoweredPHIs` lives for the whole function, so identical
    /// phis of different blocks share the temporary.
    #[test]
    fn test_identical_phis_of_different_blocks_share_one_copy_on_each_edge() {
        let phi = |result| Phi { result, incoming: vec![(0, 1), (3, 2)] };
        let was = body(vec![
            block(0, vec![def(1), branch(2), jump(1)], &[1, 2], vec![]),
            block(1, vec![read(10)], &[], vec![phi(10)]),
            block(2, vec![read(11)], &[], vec![phi(11)]),
            block(3, vec![def(2), branch(2), jump(1)], &[1, 2], vec![]),
        ]);
        let now = lowered(&was).expect("lowers");
        assert_eq!(
            copies(&now.blocks[0]).len(),
            1,
            "one copy of 1 on the edge into both: {:?}",
            copies(&now.blocks[0])
        );
        assert_eq!(copies(&now.blocks[3]).len(), 1, "{:?}", copies(&now.blocks[3]));
    }
}
