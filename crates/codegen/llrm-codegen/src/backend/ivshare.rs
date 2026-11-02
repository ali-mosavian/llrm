//! Streams that walk the same index share one pointer (DEVIATE LSR2).
//!
//! A loop that reads `[b0 + i]`, `[b1 + i]`, `[b2 + i]` holds three bases
//! and the index; the target has three registers that can make an address.
//! `getMinusSCEV` of two unrelated pointers is `CouldNotCompute` and a pointer
//! made from another object breaks provenance, so LSR cannot make the
//! difference; LIR holds integers, which have none. The sum `p = i + b0`
//! steps with `i`, and each other stream is `[p + (bk - b0)]` with the
//! difference made once, in the preheader: `k` registers instead of `k + 1`.
//! Each operand keeps its own selector, so the streams may lie in different
//! segments: the offsets wrap alike. The precedent outside the pin is gcc's
//! `get_computation_aff_1` (tree-ssa-loop-ivopts.cc#L4028).
//!
//! # How it works
//! An innermost loop with a preheader and one latch. Its index is a phi `i`
//! stepped by one `add i, imm` or `inc i`. The streams are the memory operands
//! indexed by `i` (scale 1) on a base defined outside the loop. The rewrite
//! applies where the distinct bases and the index are one more than the target
//! has registers to address with: then the pointer and the differences fit
//! them. The index has no other reader than the streams and its step; or it
//! is doubled for two-byte elements, and the double has no other reader.
//!
//! # Example
//! A loop reading `[b0 + i]`, `[b1 + i]`, `[b2 + i]`; the difference is made
//! once, ahead of the loop, and the streams share one pointer:
//!
//! ```text
//!   pre:                              pre:
//!     jmp loop                          d1 := sub b1, b0
//!   loop:                       =>      p := add i, b0
//!     x0 := [b0 + i]                    jmp loop
//!     x1 := [b1 + i]                  loop:
//!     x2 := [b2 + i]                    x0 := [p]
//!     i := add i, 1                     x1 := [p + d1]
//!                                       p := add p, 1
//! ```
//!
//! O(instructions) per loop.

use std::rc::Rc;
use std::sync::Arc;

use crate::analysis::loops as loopy;
use crate::backend::classes::RegisterClasses;
use crate::model::ir::{self, Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};

pub struct IvShare {
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for IvShare {
    fn class_name(&self) -> &'static str {
        "IvShare"
    }

    fn name(&self) -> &str {
        "ivshare"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(shared(&body, &self.classes))
    }
}

/// The index of a loop: its phi, the value it takes from the preheader, and the
/// instruction that steps it.
struct Index {
    phi: u32,
    /// The value the streams are indexed by: the phi, or its double.
    key: u32,
    /// How many of the phi's steps the key moves in one: 1 or 2.
    factor: i64,
    /// The `add phi, phi` that makes the key, where the key is not the phi.
    doubler: Option<Arc<Insn>>,
    start: u32,
    step: Arc<Insn>,
}

fn operands(one: &Insn) -> impl Iterator<Item = &Loc> {
    one.what.iter().flat_map(|what| what.dests.iter().chain(&what.sources))
}

/// `add i, imm` / `inc i` stepping the phi `phi` into `next`.
fn steps(
    one: &Insn,
    phi: u32,
) -> bool {
    let Some(what) = &one.what else { return false };
    match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (Operation::Binary, Some("add"), [Loc::Held(held), Loc::Imm(_)]) => held.value == phi,
        (Operation::Unary, Some("inc"), [Loc::Held(held)]) => held.value == phi,
        _ => false,
    }
}

fn instruction(
    beside: &Insn,
    name: &str,
    into: Held,
    from: Held,
    other: Loc,
) -> Arc<Insn> {
    let what = Semantics {
        name: Some(name.into()),
        dests: vec![Loc::Held(into)],
        sources: vec![Loc::Held(from), other],
        ..Semantics::new(Operation::Binary)
    };
    let uses: Vec<u32> = [from.value]
        .into_iter()
        .chain(match &what.sources[1] {
            Loc::Held(held) => Some(held.value),
            _ => None,
        })
        .collect();
    let covers = crate::backend::phielim::_nothing(Some(beside), beside.at);
    Arc::new(Insn::new(beside.at, Some(covers), Some(what), vec![into.value], uses))
}

/// Whether a conditional branch reads the flags `step` writes: no instruction
/// that sets them stands between, only moves and the like.
fn flags_end_the_loop(
    body: &LirBody,
    inside: &std::collections::BTreeSet<i64>,
    at_of: &HashMap<i64, usize>,
    step: &Arc<Insn>,
) -> bool {
    // Instructions that leave the flags as they are.
    const KEEPS: [&str; 8] = ["mov", "lea", "movzx", "movsx", "push", "pop", "xchg", "cwd"];
    for at in inside {
        let insns = &body.blocks[at_of[at]].insns;
        let Some(from) = insns.iter().position(|one| Arc::ptr_eq(one, step)) else { continue };
        for one in &insns[from + 1..] {
            let Some(what) = one.what.as_ref() else { return false };
            let name = what.name.as_deref().unwrap_or_default();
            if what.op == Operation::Branch {
                return name.starts_with('j') && name != "jmp";
            }
            if !KEEPS.contains(&name) {
                return false;
            }
        }
    }
    false
}

pub fn shared(
    body: &LirBody,
    classes: &RegisterClasses,
) -> LirBody {
    let found = loopy::loops(&body.blocks, Some(body.entry));
    if found.is_empty() {
        return body.clone();
    }
    let mut out = body.clone();
    let mut next = body.next_value();
    let mut changed = false;
    for lp in &found {
        let inside = lp.body.to_set();
        // Innermost: no other loop's header inside.
        if found.iter().any(|other| other.header != lp.header && inside.contains(&other.header)) {
            continue;
        }
        let [latch] = lp.latches.iter().copied().collect::<Vec<_>>()[..] else { continue };
        let at_of: HashMap<i64, usize> = out.blocks.iter().enumerate().map(|(n, b)| (b.at, n)).collect();
        let entries: Vec<i64> = out
            .blocks
            .iter()
            .filter(|b| !inside.contains(&b.at) && b.succ.contains(&lp.header))
            .map(|b| b.at)
            .collect();
        let [pre] = entries[..] else { continue };
        if out.blocks[at_of[&pre]].succ.len() != 1 {
            continue;
        }
        // The values the loop defines.
        let mut defined: HashSet<u32> = HashSet::default();
        for at in &inside {
            let block = &out.blocks[at_of[at]];
            defined.extend(block.phis.iter().map(|phi| phi.result));
            for one in block.insns.iter() {
                defined.extend(one.defines.iter().copied());
            }
        }
        let header = &out.blocks[at_of[&lp.header]];
        let mut index = None;
        for phi in header.phis.iter() {
            let [(a, va), (b, vb)] = phi.incoming[..] else { continue };
            let (start, step_value) = if a == pre && b == latch {
                (va, vb)
            } else if b == pre && a == latch {
                (vb, va)
            } else {
                continue;
            };
            let step = inside
                .iter()
                .find_map(
                    |at| out.blocks[at_of[at]]
                        .insns
                        .iter()
                        .find(|one| one.defines == [step_value] && steps(one, phi.result))
                        .map(|one| Arc::clone(one)),
                );
            if let Some(step) = step {
                index = Some(Index { phi: phi.result, key: phi.result, factor: 1, doubler: None, start, step });
                break;
            }
        }
        let Some(mut index) = index else { continue };
        // The step's flags end the loop: the index is the test, not only a
        // stream's offset, and the pointer's step would test the pointer.
        if flags_end_the_loop(&out, &inside, &at_of, &index.step) {
            continue;
        }
        // `add phi, phi`, the index of a two-byte element stepped by one.
        let doubler = inside.iter().find_map(|at| {
            out.blocks[at_of[at]].insns.iter().find(|one| {
                one.uses == [index.phi, index.phi]
                    && one.defines.len() == 1
                    && one.what.as_ref().is_some_and(|what| {
                        what.op == Operation::Binary && what.name.as_deref() == Some("add") && what.sources.len() == 2
                    })
            })
        });
        if let Some(doubler) = doubler.cloned() {
            let key = doubler.defines[0];
            let streamed = inside
                .iter()
                .any(
                    |at| out.blocks[at_of[at]]
                        .insns
                        .iter()
                        .any(
                            |one| operands(one).any(|loc| {
                                matches!(
                                    loc,
                                    Loc::Mem(cell) if cell.index.is_some_and(|h| h.value == key)
                                )
                            }),
                        ),
                );
            if streamed {
                index.key = key;
                index.factor = 2;
                index.doubler = Some(doubler);
            }
        }
        // The streams, and every other reader of the index.
        let mut bases: Vec<u32> = Vec::new();
        let mut other_readers = 0;
        for at in &inside {
            for one in out.blocks[at_of[at]].insns.iter() {
                if Arc::ptr_eq(one, &index.step)
                    || index.doubler.as_ref().is_some_and(|d| Arc::ptr_eq(one, d))
                    || !one.uses.contains(&index.key)
                {
                    continue;
                }
                let stream = operands(one).find_map(|loc| match loc {
                    Loc::Mem(cell)
                        if cell.index.is_some_and(|held| held.value == index.key)
                            && cell.scale == 1
                            && cell.base.is_some_and(|held| !defined.contains(&held.value)) =>
                    {
                        cell.base.map(|held| held.value)
                    }
                    _ => None,
                });
                match stream {
                    Some(base)
                        if operands(one)
                            .filter(|loc| ir::values(loc).into_iter().any(|h| h.value == index.key))
                            .count()
                            == 1 =>
                    {
                        if !bases.contains(&base) {
                            bases.push(base);
                        }
                    }
                    _ => other_readers += 1,
                }
            }
            if out.blocks[at_of[at]]
                .phis
                .iter()
                .any(|phi| phi.incoming.iter().any(|&(_, v)| v == index.key) && phi.result != index.key)
            {
                other_readers += 1;
            }
        }
        let scarce = classes.word_bases.union(&classes.word_indexes).count();
        // The bases and the index do not fit the registers an address is made
        // of, and the pointer and the differences do: more streams than that
        // still spill, and the preheader costs.
        if other_readers > 0 || bases.len() < 2 || bases.len() + 1 <= scarce || bases.len() > scarce {
            continue;
        }
        let width = match header.phis.iter().find(|phi| phi.result == index.phi) {
            Some(_) => 2,
            None => continue,
        };
        let held = |value: u32| Held { value, width };
        let b0 = bases[0];
        let iv = next;
        let iv_next = next + 1;
        let iv_start = next + 2;
        next += 3;
        let mut difference: HashMap<u32, u32> = HashMap::default();
        for &base in &bases[1..] {
            difference.insert(base, next);
            next += 1;
        }
        // The preheader.
        let pre_at = at_of[&pre];
        let last = out.blocks[pre_at].insns.last().cloned().expect("a terminator");
        let twice = next;
        next += 1;
        let mut made: Vec<Arc<Insn>> = if index.factor == 2 {
            vec![
                instruction(&last, "add", held(twice), held(index.start), Loc::Held(held(index.start))),
                instruction(&last, "add", held(iv_start), held(twice), Loc::Held(held(b0))),
            ]
        } else {
            vec![instruction(&last, "add", held(iv_start), held(index.start), Loc::Held(held(b0)))]
        };
        for &base in &bases[1..] {
            made.push(instruction(&last, "sub", held(difference[&base]), held(base), Loc::Held(held(b0))));
        }
        let block = &out.blocks[pre_at];
        let rebuilt = crate::backend::phielim::_before_the_terminator(block, &made);
        out.blocks[pre_at] = block.with_insns(rebuilt);
        // The loop: the streams, and the step beside the index's.
        let bumped = match &index.step.what {
            Some(what) => match what.sources.as_slice() {
                [_, Loc::Imm(imm)] => Loc::Imm(llrm_lir::Imm { value: imm.value * index.factor, ..*imm }),
                _ => Loc::Imm(llrm_lir::Imm { value: index.factor, width: 2, address: None }),
            },
            None => unreachable!(),
        };
        for at in inside.iter().copied().collect::<Vec<_>>() {
            let n = at_of[&at];
            let old: Vec<Arc<Insn>> = out.blocks[n].insns.to_vec();
            let mut rewritten: Vec<Arc<Insn>> = Vec::with_capacity(old.len() + 1);
            for one in old {
                if Arc::ptr_eq(&one, &index.step) {
                    rewritten.push(one.clone());
                    rewritten.push(instruction(&one, "add", held(iv_next), held(iv), bumped.clone()));
                    continue;
                }
                if !one.uses.contains(&index.key) {
                    rewritten.push(one);
                    continue;
                }
                let mut edited = (*one).clone();
                let mut base_of = 0;
                if let Some(what) = edited.what.as_mut() {
                    for loc in what.dests.iter_mut().chain(what.sources.iter_mut()) {
                        if let Loc::Mem(cell) = loc {
                            if cell.index.is_some_and(|h| h.value == index.key) {
                                base_of = cell.base.map_or(0, |h| h.value);
                                if base_of == b0 {
                                    cell.base = Some(held(iv));
                                    cell.index = None;
                                } else {
                                    cell.base = Some(held(iv));
                                    cell.index = Some(held(difference[&base_of]));
                                }
                            }
                        }
                    }
                }
                let still: HashSet<u32> = operands(&edited)
                    .flat_map(|loc| ir::values(loc).into_iter().map(|h| h.value).collect::<Vec<_>>())
                    .collect();
                let mut uses: Vec<u32> = Vec::new();
                for &value in edited.uses.iter() {
                    let swapped = if value == index.key {
                        iv
                    } else if value == base_of && !still.contains(&value) {
                        if base_of == b0 { iv } else { difference[&base_of] }
                    } else {
                        value
                    };
                    if !uses.contains(&swapped) {
                        uses.push(swapped);
                    }
                }
                for &value in &still {
                    if !uses.contains(&value) && (value == iv || difference.values().any(|&d| d == value)) {
                        uses.push(value);
                    }
                }
                edited.uses = uses;
                rewritten.push(Arc::new(edited));
            }
            out.blocks[n] = out.blocks[n].with_insns(rewritten);
        }
        // The phi of the pointer; the index and its step stay for dead code
        // elimination where nothing reads them.
        let header_at = at_of[&lp.header];
        let mut phis = out.blocks[header_at].phis.to_vec();
        phis.push(Phi { result: iv, incoming: vec![(pre, iv_start), (latch, iv_next)] });
        let block = &mut out.blocks[header_at];
        *block = LirBlock { phis: phis.into(), ..(**block).clone() }.into();
        // The index and its step, or the double, where nothing reads them now.
        let reads = |value: u32, out: &LirBody, skip: &Insn| {
            out.blocks
                .iter()
                .any(
                    |block| block.insns.iter().any(|one| !std::ptr::eq(&**one, skip) && one.uses.contains(&value))
                        || block
                            .phis
                            .iter()
                            .any(|phi| phi.result != index.phi && phi.incoming.iter().any(|&(_, v)| v == value)),
                )
        };
        let gone: Option<(Arc<Insn>, Option<u32>)> = match &index.doubler {
            Some(doubler) if !reads(index.key, &out, doubler) => Some((Arc::clone(doubler), None)),
            None if !reads(index.phi, &out, &index.step) && !reads(index.step.defines[0], &out, &index.step) => {
                Some((Arc::clone(&index.step), Some(index.phi)))
            }
            _ => None,
        };
        if let Some((dead, phi)) = gone {
            for block in &mut out.blocks {
                if block.insns.iter().any(|one| Arc::ptr_eq(one, &dead)) {
                    let kept: Vec<Arc<Insn>> =
                        block.insns.iter().filter(|one| !Arc::ptr_eq(one, &dead)).cloned().collect();
                    *block = block.with_insns(kept);
                }
                if let Some(phi) = phi {
                    if block.at == lp.header {
                        let kept: Vec<Phi> = block.phis.iter().filter(|one| one.result != phi).cloned().collect();
                        *block = LirBlock { phis: kept.into(), ..(**block).clone() }.into();
                    }
                }
            }
        }
        changed = true;
    }
    if changed { out } else { body.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::Mem;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 2 })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    /// `total += a[i] + b[i] + c[i]` over three arrays: `streams` bases `1..`,
    /// the index `10` starting at `5`, stepped by 2 into `11`.
    fn sum_loop(streams: u32) -> LirBody {
        sum_loop_ending(streams, true)
    }

    /// [`sum_loop`], the branch reading the step's flags where not `compared`.
    fn sum_loop_ending(
        streams: u32,
        compared: bool,
    ) -> LirBody {
        let cell = |base: u32| {
            Loc::Mem(Mem {
                base: Some(Held { value: base, width: 2 }),
                index: Some(Held { value: 10, width: 2 }),
                ..Mem::new(None, 2)
            })
        };
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let mut loop_insns = Vec::new();
        for n in 0..streams {
            loop_insns.push(insn(
                10 + i64::from(n),
                Operation::Binary,
                "add",
                vec![held(20 + n)],
                vec![held(if n == 0 { 30 } else { 19 + n }), cell(1 + n)],
                vec![20 + n],
                vec![if n == 0 { 30 } else { 19 + n }, 1 + n, 10],
            ));
        }
        loop_insns.push(insn(
            20,
            Operation::Binary,
            "add",
            vec![held(11)],
            vec![held(10), Loc::Imm(llrm_lir::Imm { value: 2, width: 2, address: None })],
            vec![11],
            vec![10],
        ));
        if compared {
            loop_insns.push(insn(
                21,
                Operation::Binary,
                "cmp",
                vec![],
                vec![held(30), Loc::Imm(llrm_lir::Imm { value: 0, width: 2, address: None })],
                vec![],
                vec![30],
            ));
        }
        loop_insns.push(insn(22, Operation::Branch, "jne", vec![], vec![], vec![], vec![]));
        let mut loop_block = block(1, vec![1, 2], loop_insns);
        loop_block.phis = vec![
            Phi { result: 10, incoming: vec![(0, 5), (1, 11)] },
            Phi { result: 30, incoming: vec![(0, 6), (1, 19 + streams)] },
        ]
        .into();
        let mut body = LirBody::new(
            "sum",
            0,
            vec![
                block(0, vec![1], vec![insn(0, Operation::Jump, "jmp", vec![], vec![], vec![], vec![])]).into(),
                loop_block.into(),
                block(2, vec![], vec![insn(30, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = (1..=6).collect();
        body
    }

    fn indexed(body: &LirBody) -> usize {
        body.blocks[1]
            .insns
            .iter()
            .flat_map(|one| operands(one))
            .filter(|loc| {
                matches!(
                    loc,
                    Loc::Mem(cell) if cell.index.is_some_and(|held| held.value == 10)
                )
            })
            .count()
    }

    /// sum_three (BASIC, -O2): three `[bx+base]` streams on one index kept
    /// three bases and the index, 266 B and 165 clk against main's 252 and
    /// 152, which kept `[bx]`, `[bx+di]`, `[bx+si]`. The streams are
    /// `[p]`, `[p+d1]`, `[p+d2]` and the index is gone from the loop.
    #[test]
    fn test_three_streams_on_one_index_share_one_pointer() {
        let classes = RegisterClasses::m16();
        let done = shared(&sum_loop(3), &classes);
        assert_eq!(indexed(&done), 0, "no stream is indexed by the old index");
        let names: Vec<_> = done.blocks[1]
            .insns
            .iter()
            .filter_map(|one| Some(one.what.as_ref()?.name.as_deref()?.to_owned()))
            .collect();
        assert_eq!(
            names.iter().filter(|name| name.as_str() == "add").count(),
            4,
            "three loads and one step: {names:?}"
        );
        assert_eq!(done.blocks[1].phis.len(), 2, "the pointer replaces the index");
        let pre: Vec<_> = done.blocks[0]
            .insns
            .iter()
            .filter_map(|one| Some(one.what.as_ref()?.name.as_deref()?.to_owned()))
            .collect();
        assert_eq!(pre, ["add", "sub", "sub", "jmp"]);
    }

    /// tests/run huffman_decode (-O2, m16) crashed: `build`'s loop ended on the
    /// flags of its zero-ending index step (`add bx, 2; je`). The rewrite took
    /// the step out, nothing reading its value, and the branch read the flags
    /// of the pointer's step, which test the pointer against zero. A step
    /// whose flags a branch reads is left alone.
    #[test]
    fn test_an_index_step_whose_flags_end_the_loop_is_left_alone() {
        let classes = RegisterClasses::m16();
        let done = shared(&sum_loop_ending(3, false), &classes);
        assert_eq!(indexed(&done), 3, "the streams keep the index");
        assert_eq!(done.blocks[1].phis.len(), 2);
    }

    /// Two streams and the index fit the three address registers: left alone.
    #[test]
    fn test_two_streams_that_fit_the_address_registers_are_left_alone() {
        let classes = RegisterClasses::m16();
        let body = sum_loop(2);
        let done = shared(&body, &classes);
        assert_eq!(indexed(&done), 2);
    }

    /// Four streams still spill the pointer and its three differences:
    /// the rewrite would only cost the preheader (four arrays, BASIC, -O2:
    /// 969 B became 980).
    #[test]
    fn test_four_streams_that_would_still_spill_are_left_alone() {
        let classes = RegisterClasses::m16();
        let done = shared(&sum_loop(4), &classes);
        assert_eq!(indexed(&done), 4);
    }
}
