//! A loop's index ends at zero (DEVIATE LSR3).
//!
//! A counted loop that tests `i != K` after its step reads `i` only as an
//! address, so `j = i - K` does the same: the bound disappears, the step's own
//! flags end the loop, and each address that used `i` takes `K` more in its
//! displacement. The pin's LoopStrengthReduce does this for every loop
//! (`ICmpZero` uses); ours reduces the innermost loops only
//! ([`crate::lsr_pass`]), and outer-loop LSR grew rows its register model could
//! not see (the inner body's other live values). Here `j` replaces `i`, so
//! the registers held are the same by construction.
//!
//! ```text
//!   bx = 0                          bx = -9
//!   L: movzx ecx, [data+bx]    =>   L: movzx ecx, [data+9+bx]
//!      ...                             ...
//!      inc bx                          inc bx
//!      cmp bx, 9                       jne L
//!      jne L
//! ```
//!
//! # Example
//! The loop above: `cmp bx, 9` goes, `bx` starts at -9, and `data` takes 9 more
//! in its displacement.
//!
//! # How it works
//! A loop of one latch and a preheader that leaves by one jump. The latch ends
//! `step; sub t, step, K; jcc header` with `t` read by nobody, `step` an `inc`,
//! `dec`, `add` or `sub` of the loop's phi `i` by a constant, `jcc` an equality
//! test (`peepholeopt::condition_of`), reading only flags the step writes. `i`
//! starts at a constant. Every other reader of `i` is a memory operand
//! naming it as base or index, whose displacement, shifted by `K` (times the
//! scale for an index), still fits the address and the encoding it had: a
//! frame cell is left, and so is a displacement that would grow from one byte
//! to the full width. `step` is read by the test and the phi only, and `i`
//! by no instruction outside the loop. The phi becomes `j` starting at
//! `start - K`, the test is gone.
//!
//! Bytes: the compare goes (2 or 3 B); the start is a `mov` of an immediate
//! either way, one byte more than `xor` of zero. Never larger at any level.
//!
//! Complexity: O(instructions) per loop.
//!
//! DEVIATE LSR3 (owner w12, exit: the pin's LSR over outer loops once its
//! register model sees the inner body's pressure, and the allocator
//! splits what the pin's does): no pin mechanism at this level; precedent
//! LLVM's `ICmpZero` formulae in LSR and gcc's `iv_elimination_compare`.
//!
//! # Runs
//! A machine phase after `IvShare`, at every -O level but -O0.

use std::rc::Rc;
use std::sync::Arc;

use crate::analysis::loops as loopy;
use crate::backend::classes::RegisterClasses;
use crate::backend::liveness;
use crate::model::ir::{Held, Loc, Operation};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashMap;

pub struct IvZero {
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for IvZero {
    fn class_name(&self) -> &'static str {
        "IvZero"
    }

    fn name(&self) -> &str {
        "ivzero"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(zeroed(&body))
    }
}

/// `value` as a signed number of `bytes` bytes.
fn wrap(
    value: i64,
    bytes: u32,
) -> i64 {
    let shift = 64 - 8 * bytes.min(8);
    (value << shift) >> shift
}

/// The displacement `cell` has with `shift` more, if it fits the address and
/// the encoding the cell had.
fn shifted(
    cell: &llrm_lir::Mem,
    shift: i64,
    bits: u32,
) -> Option<llrm_lir::Mem> {
    let mut out = cell.clone();
    let (old, new) = match cell.addr {
        Some(addr) => {
            out.addr = Some(addr.plus(shift));
            (addr.disp, addr.disp + shift)
        }
        None => {
            out.offset += shift;
            (cell.offset, cell.offset + shift)
        }
    };
    let limit = 1i64 << (bits.min(32) - 1);
    if new < -limit || new >= limit {
        return None;
    }
    // A displacement that takes more bytes to encode than it did grows the
    // form: a cell of a symbol always has the full width; a register-based
    // one takes none for zero, one when it fits a byte, the full width past
    // that; a frame cell is bp-based, so one byte at the least.
    let bytes = |disp: i64| -> u32 {
        match cell.addr {
            Some(addr) if !matches!(addr.space, llrm_lir::Space::Frame | llrm_lir::Space::Literal) => bits / 8,
            Some(addr) if addr.space == llrm_lir::Space::Frame && (-128..128).contains(&disp) => 1,
            Some(_) | None if disp == 0 => 0,
            Some(_) | None if (-128..128).contains(&disp) => 1,
            _ => bits / 8,
        }
    };
    if bytes(new) > bytes(old) {
        return None;
    }
    Some(out)
}

/// Whether `one` reads `value`: as it lists its uses or in an operand.
fn names(
    one: &Insn,
    value: u32,
) -> bool {
    one.uses.contains(&value)
        || one.what.as_ref().is_some_and(|w| {
            w.dests
                .iter()
                .chain(&w.sources)
                .any(|loc| crate::model::ir::values(loc).into_iter().any(|held| held.value == value))
        })
}

pub fn zeroed(body: &LirBody) -> LirBody {
    let Some(regs) = body.registers else { return body.clone() };
    let found = loopy::loops(&body.blocks, Some(body.entry));
    if found.is_empty() {
        return body.clone();
    }
    let mut out = body.clone();
    let mut next = body.next_value();
    let mut changed = false;
    for lp in &found {
        let inside = lp.body.to_set();
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
        let latch_block = &out.blocks[at_of[&latch]];
        let insns: Vec<Arc<Insn>> = latch_block.insns.iter().cloned().collect();
        let Some(branch) = insns.iter().rposition(|one| one.what.as_ref().is_some_and(|w| w.op == Operation::Branch))
        else {
            continue;
        };
        if branch < 2 {
            continue;
        }
        let equality = insns[branch]
            .what
            .as_ref()
            .and_then(|w| w.name.as_deref())
            .and_then(crate::backend::peepholeopt::condition_of)
            .is_some_and(|condition| matches!(condition, "e" | "z" | "ne" | "nz"));
        if !equality {
            continue;
        }
        let (test, step) = (&insns[branch - 1], &insns[branch - 2]);
        let Some(tw) = test.what.as_ref() else { continue };
        let ([Loc::Held(_)], [Loc::Held(stepped), Loc::Imm(bound)]) = (tw.dests.as_slice(), tw.sources.as_slice())
        else {
            continue;
        };
        if tw.name.as_deref() != Some("sub") || tw.op != Operation::Binary || bound.address.is_some() {
            continue;
        }
        let Some(sw) = step.what.as_ref() else { continue };
        let step_ok = step.defines == [stepped.value] && matches!(sw.name.as_deref(), Some("inc" | "dec"))
            || matches!(
                (sw.name.as_deref(), sw.sources.as_slice()),
                (Some("add" | "sub"), [_, Loc::Imm(c)]) if c.address.is_none()
            );
        let (Some(Loc::Held(index)), true) = (sw.sources.first(), step_ok && step.defines == [stepped.value]) else {
            continue;
        };
        // The flags the test reads are the step's too.
        let flags_ok = liveness::effect(regs, body.bits, &insns[branch])
            .zip(liveness::effect(regs, body.bits, step))
            .is_some_and(|(branch_effect, step_effect)| {
                let reads = branch_effect.all_reads().mask();
                reads != 0 && reads & !step_effect.writes.mask() == 0
            });
        if !flags_ok {
            continue;
        }
        let header = &out.blocks[at_of[&lp.header]];
        let Some(phi) = header.phis.iter().find(|p| p.result == index.value).cloned() else { continue };
        let Some(&(_, start)) = phi.incoming.iter().find(|&&(from, _)| from == pre) else { continue };
        if !phi.incoming.iter().any(|&(from, v)| from == latch && v == stepped.value) {
            continue;
        }
        // The start: a constant, through copies.
        let mut chain: Vec<Arc<Insn>> = Vec::new();
        let mut at_value = start;
        let first = loop {
            let defs: Vec<&Arc<Insn>> =
                out.blocks.iter().flat_map(|b| b.insns.iter()).filter(|one| one.defines == [at_value]).collect();
            let [def] = defs[..] else { break None };
            let Some(dw) = def.what.as_ref() else { break None };
            if dw.op != Operation::Move || chain.len() > 4 {
                break None;
            }
            chain.push(Arc::clone(def));
            match (dw.dests.as_slice(), dw.sources.as_slice()) {
                ([Loc::Held(_)], [Loc::Imm(first)]) if first.address.is_none() => break Some(first.clone()),
                ([Loc::Held(_)], [Loc::Held(from)]) => at_value = from.value,
                _ => break None,
            }
        };
        let Some(first) = first else { continue };
        let start_def = Arc::clone(&chain[0]);
        let start_def = &start_def;
        // Bytes: the compare goes (opcode, modrm and the immediate: one byte
        // where it fits, the index's width past that, a prefix where the index
        // is not the mode's width); a start of zero was `xor r, r` (2 B) and is
        // a `mov` of an immediate now. Only a strict gain is taken, at every
        // level: a 32-bit index starting at zero comes out even.
        let width_bytes = i64::from(index.width);
        let mode_bytes = i64::from(body.bits / 8);
        let compare =
            2 + if (-128..128).contains(&bound.value) { 1 } else { width_bytes } + i64::from(width_bytes != mode_bytes);
        let start_cost = if first.value == 0 { 1 + width_bytes - 2 } else { 0 };
        if width_bytes > 4 || start_cost - compare >= 0 {
            continue;
        }
        // Readers of the index and of the stepped value.
        let mut feasible = true;
        let mut edits: Vec<(i64, usize, Arc<Insn>)> = Vec::new();
        let mut index_reads_outside = false;
        let mut stepped_reads = 0;
        for block in &out.blocks {
            let inside_loop = inside.contains(&block.at);
            for (n, one) in block.insns.iter().enumerate() {
                if Arc::ptr_eq(one, step) || Arc::ptr_eq(one, test) {
                    continue;
                }
                if names(one, stepped.value) {
                    stepped_reads += 1;
                }
                if !names(one, index.value) {
                    continue;
                }
                if !inside_loop {
                    index_reads_outside = true;
                    continue;
                }
                if one.what.is_none() {
                    feasible = false;
                    continue;
                }
                let mut edited = (**one).clone();
                let mut addressed = false;
                let ew = edited.what.as_mut().unwrap();
                for loc in ew.dests.iter_mut().chain(ew.sources.iter_mut()) {
                    match loc {
                        Loc::Mem(cell)
                            if cell.base.is_some_and(|h| h.value == index.value)
                                || cell.index.is_some_and(|h| h.value == index.value) =>
                        {
                            addressed = true;
                            let mut shift = 0;
                            if cell.base.is_some_and(|h| h.value == index.value) {
                                shift += bound.value;
                            }
                            if cell.index.is_some_and(|h| h.value == index.value) {
                                shift += bound.value * cell.scale;
                            }
                            match shifted(cell, shift, body.bits) {
                                Some(moved) => *cell = moved,
                                None => feasible = false,
                            }
                        }
                        Loc::Held(h) if h.value == index.value => feasible = false,
                        Loc::Mem(cell) if cell.selector.is_some_and(|h| h.value == index.value) => feasible = false,
                        _ => {}
                    }
                }
                // A reader that names it in no address (a call's argument)
                // needs the value itself.
                if !addressed || !edited.requires.is_empty() {
                    feasible = false;
                }
                edits.push((block.at, n, Arc::new(edited)));
            }
            for p in block.phis.iter() {
                if p.result != index.value && p.incoming.iter().any(|&(_, v)| v == index.value) {
                    feasible = false;
                }
                if p.result != index.value && p.incoming.iter().any(|&(_, v)| v == stepped.value) {
                    stepped_reads += 1;
                }
            }
        }
        if !feasible || index_reads_outside || stepped_reads > 0 || edits.is_empty() {
            continue;
        }
        // Make the change.
        let width = index.width;
        let j = next;
        let j_next = next + 1;
        let j0 = next + 2;
        next += 3;
        let held = |value: u32| Held { value, width };
        // The start in the preheader.
        let pre_at = at_of[&pre];
        let last = out.blocks[pre_at].insns.last().cloned().expect("a terminator");
        let mut made = (**start_def).clone();
        {
            let w = made.what.as_mut().unwrap();
            w.dests = vec![Loc::Held(held(j0))];
            w.sources = vec![Loc::Imm(llrm_lir::Imm {
                value: wrap(first.value - bound.value, width),
                width: first.width,
                address: None,
            })];
            made.defines = vec![j0];
            made.uses = Vec::new();
            made.at = last.at;
            made.covers = Some(crate::backend::phielim::_nothing(Some(&last), last.at));
        }
        let block = &out.blocks[pre_at];
        let rebuilt = crate::backend::phielim::_before_the_terminator(block, &[Arc::new(made)]);
        out.blocks[pre_at] = block.with_insns(rebuilt);
        // The start's old definition goes where nothing else reads it.
        let start_reads = out
            .blocks
            .iter()
            .map(|b| {
                b.insns.iter().filter(|one| one.uses.contains(&start)).count()
                    + b.phis.iter().filter(|p| p.incoming.iter().any(|&(_, v)| v == start)).count()
            })
            .sum::<usize>();
        let drop_start = start_reads == 1;
        // The copies the start came through go as far as nothing else reads
        // them.
        let mut dropped: Vec<Arc<Insn>> = Vec::new();
        if drop_start {
            dropped.push(Arc::clone(&chain[0]));
            for pair in chain.windows(2) {
                let value = pair[1].defines[0];
                let reads = out
                    .blocks
                    .iter()
                    .map(|b| {
                        b.insns.iter().filter(|one| one.uses.contains(&value)).count()
                            + b.phis.iter().filter(|p| p.incoming.iter().any(|&(_, v)| v == value)).count()
                    })
                    .sum::<usize>();
                if reads == 1 {
                    dropped.push(Arc::clone(&pair[1]));
                } else {
                    break;
                }
            }
        }
        // The loop: the readers, the step, the test.
        let mut by_block: HashMap<i64, Vec<(usize, Arc<Insn>)>> = HashMap::default();
        for (at, n, one) in edits {
            by_block.entry(at).or_default().push((n, one));
        }
        let swap = |value: u32| if value == index.value { j } else { value };
        for block in out.blocks.iter_mut() {
            let edits = by_block.remove(&block.at).unwrap_or_default();
            let editing = !edits.is_empty()
                || block.at == latch
                || block.insns.iter().any(|one| dropped.iter().any(|d| Arc::ptr_eq(one, d)));
            if !editing {
                continue;
            }
            let mut rewritten: Vec<Arc<Insn>> = Vec::with_capacity(block.insns.len());
            for (n, one) in block.insns.iter().enumerate() {
                if Arc::ptr_eq(one, test) {
                    continue;
                }
                if dropped.iter().any(|d| Arc::ptr_eq(one, d)) {
                    continue;
                }
                if Arc::ptr_eq(one, step) {
                    let mut stepped_insn = (**one).clone();
                    let w = stepped_insn.what.as_mut().unwrap();
                    w.dests = vec![Loc::Held(held(j_next))];
                    if let Some(Loc::Held(h)) = w.sources.first_mut() {
                        h.value = j;
                    }
                    stepped_insn.defines = vec![j_next];
                    stepped_insn.uses = stepped_insn.uses.iter().map(|&v| swap(v)).collect();
                    rewritten.push(Arc::new(stepped_insn));
                    continue;
                }
                if let Some((_, edited)) = edits.iter().find(|(m, _)| *m == n) {
                    let mut e = (**edited).clone();
                    let ew = e.what.as_mut().unwrap();
                    for loc in ew.dests.iter_mut().chain(ew.sources.iter_mut()) {
                        if let Loc::Mem(cell) = loc {
                            for part in [&mut cell.base, &mut cell.index] {
                                if let Some(h) = part {
                                    if h.value == index.value {
                                        h.value = j;
                                    }
                                }
                            }
                        }
                    }
                    let mut uses: Vec<u32> = Vec::new();
                    for &v in e.uses.iter() {
                        let v = swap(v);
                        if !uses.contains(&v) {
                            uses.push(v);
                        }
                    }
                    e.uses = uses;
                    rewritten.push(Arc::new(e));
                    continue;
                }
                rewritten.push(Arc::clone(one));
            }
            *block = block.with_insns(rewritten);
        }
        // The phi.
        let header_at = at_of[&lp.header];
        let phis: Vec<Phi> = out.blocks[header_at]
            .phis
            .iter()
            .map(|p| {
                if p.result == index.value {
                    Phi {
                        result: j,
                        incoming: p
                            .incoming
                            .iter()
                            .map(|&(from, v)| {
                                if from == pre {
                                    (from, j0)
                                } else {
                                    (from, if v == stepped.value { j_next } else { v })
                                }
                            })
                            .collect(),
                    }
                } else {
                    p.clone()
                }
            })
            .collect();
        let block = &mut out.blocks[header_at];
        *block = LirBlock { phis: phis.into(), ..(**block).clone() }.into();
        changed = true;
    }
    if changed { out } else { body.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Mem, Semantics};
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 2 })
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(llrm_lir::Imm { value, width: 2, address: None })
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
        let target = matches!(op, Operation::Branch | Operation::Jump).then_some(1);
        let what = Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    /// `for (i = 0; i != 9; i++) sum += data[i]` as the latch leaves it:
    /// `inc i; sub t, i, 9; jne`. `step` and `branch` pick the other shapes;
    /// `extra` is an instruction in the loop that reads `i`.
    fn counted(
        disp: i64,
        plain: bool,
        step: (&str, Vec<Loc>),
        branch: &str,
        extra: Option<Arc<Insn>>,
    ) -> LirBody {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let cell = Loc::Mem(if plain {
            Mem { base: Some(Held { value: 2, width: 2 }), offset: disp, ..Mem::new(None, 1) }
        } else {
            Mem {
                base: Some(Held { value: 2, width: 2 }),
                ..Mem::new(Some(llrm_lir::Addr::new(llrm_lir::Space::Segment, disp)), 1)
            }
        });
        let mut loop_insns = vec![insn(10, Operation::Extend, "movzx", vec![held(3)], vec![cell], vec![3], vec![2])];
        loop_insns.extend(extra);
        loop_insns.push(insn(11, Operation::Unary, step.0, vec![held(4)], step.1, vec![4], vec![2]));
        loop_insns.push(insn(12, Operation::Binary, "sub", vec![held(5)], vec![held(4), imm(9)], vec![5], vec![4]));
        loop_insns.push(insn(13, Operation::Branch, branch, vec![], vec![], vec![], vec![]));
        loop_insns.push(insn(14, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]));
        let mut head = block(1, vec![1, 2], loop_insns);
        head.phis = vec![Phi { result: 2, incoming: vec![(0, 1), (1, 4)] }].into();
        let mut body = LirBody::new(
            "counted",
            0,
            vec![
                block(
                    0,
                    vec![1],
                    vec![
                        insn(0, Operation::Move, "mov", vec![held(1)], vec![imm(0)], vec![1], vec![]),
                        insn(1, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]),
                    ],
                )
                .into(),
                head.into(),
                block(2, vec![], vec![insn(20, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body
    }

    fn names(
        body: &LirBody,
        at: usize,
    ) -> Vec<String> {
        body.blocks[at].insns.iter().filter_map(|one| Some(one.what.as_ref()?.name.as_deref()?.to_owned())).collect()
    }

    fn inc() -> (&'static str, Vec<Loc>) {
        ("inc", vec![held(2)])
    }

    /// crc, the outer loop (c, nib; -O2/-Os): `inc bx; cmp bx,9; jne` where
    /// 854 ended the index at zero: 1 clock a trip, 3 B, against main.
    #[test]
    fn test_a_counted_loops_bound_goes_and_the_index_starts_at_minus_the_bound() {
        let done = zeroed(&counted(100, false, inc(), "jne", None));
        assert_eq!(names(&done, 1), ["movzx", "inc", "jne", "jmp"], "the compare is gone");
        assert_eq!(names(&done, 0), ["mov", "jmp"]);
        let Some(Loc::Imm(start)) = done.blocks[0].insns[0].what.as_ref().map(|w| w.sources[0].clone()) else {
            panic!("the start is an immediate")
        };
        assert_eq!(start.value, -9);
        let Some(Loc::Mem(cell)) = done.blocks[1].insns[0].what.as_ref().map(|w| w.sources[0].clone()) else {
            panic!("a load")
        };
        assert_eq!(cell.addr.map(|addr| addr.disp), Some(109), "the displacement takes the bound");
    }

    /// A step by two (`add`) ends the same way.
    #[test]
    fn test_a_step_by_two_ends_at_zero_too() {
        let done = zeroed(&counted(100, false, ("add", vec![held(2), imm(2)]), "jne", None));
        assert_eq!(names(&done, 1), ["movzx", "add", "jne", "jmp"]);
    }

    /// A displacement that would no longer fit the address is left alone.
    #[test]
    fn test_a_displacement_that_would_overflow_is_not_shifted() {
        let before = counted(32765, false, inc(), "jne", None);
        let done = zeroed(&before);
        assert_eq!(names(&done, 1), ["movzx", "inc", "sub", "jne", "jmp"]);
    }

    /// An index read as a value (not an address) cannot become `i - K`.
    #[test]
    fn test_an_index_read_as_a_value_is_left_alone() {
        let reader = insn(15, Operation::Binary, "add", vec![held(6)], vec![held(3), held(2)], vec![6], vec![3, 2]);
        let done = zeroed(&counted(100, false, inc(), "jne", Some(reader)));
        assert_eq!(names(&done, 1), ["movzx", "add", "inc", "sub", "jne", "jmp"]);
    }

    /// `i < K` is not `j != 0`: an ordered test keeps its compare.
    #[test]
    fn test_an_ordered_exit_test_is_left_alone() {
        let done = zeroed(&counted(100, false, inc(), "jl", None));
        assert_eq!(names(&done, 1), ["movzx", "inc", "sub", "jl", "jmp"]);
    }

    /// particle (nib, -O2): five `[bx+di]` cells with no displacement took a
    /// two-byte one each from the shift: +9 B against a three-byte compare
    /// gone. A displacement that takes more bytes than it did is not shifted.
    #[test]
    fn test_a_cell_with_no_displacement_that_would_need_one_is_left_alone() {
        let done = zeroed(&counted(0, true, inc(), "jne", None));
        assert_eq!(names(&done, 1), ["movzx", "inc", "sub", "jne", "jmp"]);
    }

    /// gfxdev.c `_planar_read` (BASIC runtime, -Os): a call took the index as
    /// an argument; the pass made `i - K` of it and left the call reading a
    /// value nothing defined ("read but never defined").
    #[test]
    fn test_an_index_a_call_takes_as_an_argument_is_left_alone() {
        let call = insn(15, Operation::Call, "call", vec![], vec![], vec![], vec![2]);
        let done = zeroed(&counted(100, false, inc(), "jne", Some(call)));
        assert_eq!(names(&done, 1), ["movzx", "call", "inc", "sub", "jne", "jmp"]);
    }
}
