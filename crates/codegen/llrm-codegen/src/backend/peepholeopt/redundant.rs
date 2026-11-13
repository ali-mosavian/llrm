//! A compare whose flags an earlier compare or subtract of the same block
//! already holds.
//!
//! Translated from the redundant-flag half of
//! `X86InstrInfo::optimizeCompareInstr`
//! ([X86InstrInfo.cpp#L5473](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5473)):
//! [`matching`] is `isRedundantFlagInstr`
//! ([L4949](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L4949)),
//! [`replacement`] is the forward scan's condition rewrite
//! ([L5726-L5800](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5726)).
//!
//! # How it works
//! `cmp a,b` after `cmp a,b` or `sub d,a,b` with no flag writer between them
//! is the same flags. `cmp b,a` is the same flags with every reader's
//! condition swapped (`jl` reads `jg`). `cmp x,C` after `cmp x,C+-1` is the
//! same flags with a reader of `x < C+1` reading `x <= C`: the eight rows of
//! the pin's table, each refused at the edge of the width where the constant
//! would wrap. Every reader before the flags are written again must name a
//! condition; any other reader (`adc`, `sbb`) or flags live into a successor
//! keeps the compare.
//!
//! Not translated: the backward scan into a single predecessor, `test`
//! against an earlier `test`, LZCNT/TZCNT, POPCNT and the NF variants (the
//! 486 and the Pentium have none of them).
//!
//! # Example
//! ```text
//!  cmp v1, v2 ; jl A        cmp v1, v2 ; jl A
//!  ...                =>    ...
//!  cmp v2, v1 ; jg B        jl B
//! ```
//!
//! From `test_a_swapped_compare_is_the_earlier_one_with_the_condition_swapped`.
//!
//! # Runs
//! In `PeepholeOptimizer`, ahead of the compare-with-zero reuse.

use std::sync::Arc;

use super::{condition_of, flag_effects, successor_overwrites_flags};
use crate::backend::lanes::Lanes;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};

/// What the earlier flag writer is to the compare.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Same {
    /// The same operands.
    Plain,
    /// The operands in the other order (`IsSwapped`).
    Swapped,
    /// The same register, a constant `delta` away: the compare's constant is
    /// the earlier one's plus `delta` (`ImmDelta` is the negative of that: the
    /// pin sets -1 when the compare's constant is one less than the earlier's).
    Delta(i64),
}

/// The operands of a compare: two values, or a value and a constant.
#[derive(Clone, Copy, PartialEq)]
enum Operands {
    Values(u32, u32, u32),
    Constant(u32, u32, i64),
}

/// `one` as a compare of its operands: a `cmp`, or a `sub` that sets the same
/// flags (its result is somebody's own business).
fn operands_of(one: &Insn) -> Option<Operands> {
    let what = one.what.as_ref()?;
    if what.indirect || one.group.is_some() {
        return None;
    }
    let name = what.name.as_deref()?;
    let shape = match (what.op, name) {
        (Operation::Compare, "cmp") if what.dests.is_empty() => &what.sources[..],
        (Operation::Binary, "sub") => &what.sources[..],
        _ => return None,
    };
    match shape {
        [Loc::Held(a), Loc::Held(b)] if a.width == b.width => Some(Operands::Values(a.value, b.value, a.width)),
        [Loc::Held(a), Loc::Imm(c)] if c.address.is_none() => Some(Operands::Constant(a.value, a.width, c.value)),
        _ => None,
    }
}

/// `isRedundantFlagInstr`: how the flags of `earlier` are those of `compare`.
fn matching(
    compare: Operands,
    earlier: Operands,
) -> Option<Same> {
    match (compare, earlier) {
        (Operands::Values(a, b, w), Operands::Values(oa, ob, ow)) if w == ow => {
            if (a, b) == (oa, ob) {
                Some(Same::Plain)
            } else if (a, b) == (ob, oa) {
                Some(Same::Swapped)
            } else {
                None
            }
        }
        (Operands::Constant(a, w, c), Operands::Constant(oa, ow, oc)) if a == oa && w == ow => {
            let (c, oc) = (c as u64, oc as u64);
            if c == oc {
                Some(Same::Plain)
            } else if c == oc.wrapping_sub(1) {
                Some(Same::Delta(-1))
            } else if c == oc.wrapping_add(1) {
                Some(Same::Delta(1))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The canonical condition and prefix of a conditional instruction's name.
fn split(name: &str) -> Option<(&'static str, &'static str)> {
    let prefix = ["cmov", "set", "j"].into_iter().find(|prefix| name.starts_with(prefix))?;
    let condition = match condition_of(name)? {
        "e" | "z" => "e",
        "ne" | "nz" => "ne",
        "l" | "nge" => "l",
        "ge" | "nl" => "ge",
        "le" | "ng" => "le",
        "g" | "nle" => "g",
        "b" | "c" | "nae" => "b",
        "ae" | "nb" | "nc" => "ae",
        "be" | "na" => "be",
        "a" | "nbe" => "a",
        "s" => "s",
        "ns" => "ns",
        "o" => "o",
        "no" => "no",
        "p" | "pe" => "p",
        "np" | "po" => "np",
        _ => return None,
    };
    Some((prefix, condition))
}

/// `getSwappedCondition`: the condition that reads `b ? a` as `a ? b` does.
fn swapped(condition: &str) -> Option<&'static str> {
    Some(match condition {
        "e" => "e",
        "ne" => "ne",
        "l" => "g",
        "le" => "ge",
        "g" => "l",
        "ge" => "le",
        "b" => "a",
        "be" => "ae",
        "a" => "b",
        "ae" => "be",
        _ => return None,
    })
}

/// The pin's `ImmDelta` rows: the condition that, read of the compare's
/// constant `value` at `width` bytes, reads the earlier compare's flags.
fn after_delta(
    condition: &str,
    delta: i64,
    value: i64,
    width: u32,
) -> Option<&'static str> {
    let bits = width * 8;
    let signed_min = -(1_i128 << (bits - 1));
    let signed_max = (1_i128 << (bits - 1)) - 1;
    let unsigned_max = (1_i128 << bits) - 1;
    let signed = i128::from(value);
    let unsigned = i128::from(value) & unsigned_max;
    Some(match condition {
        "l" if delta == 1 && signed != signed_min => "le",
        "b" if delta == 1 && unsigned != 0 => "be",
        "ge" if delta == 1 && signed != signed_min => "g",
        "ae" if delta == 1 && unsigned != 0 => "a",
        "g" if delta == -1 && signed != signed_max => "ge",
        "a" if delta == -1 && unsigned != unsigned_max => "ae",
        "le" if delta == -1 && signed != signed_max => "l",
        "be" if delta == -1 && unsigned != unsigned_max => "b",
        _ => return None,
    })
}

/// The condition a reader named `condition` takes once the compare is gone;
/// none where the pin keeps the compare.
fn replacement(
    condition: &'static str,
    same: Same,
    compare: Operands,
) -> Option<&'static str> {
    match same {
        Same::Plain => Some(condition),
        Same::Swapped => swapped(condition),
        Same::Delta(delta) => {
            let Operands::Constant(_, width, value) = compare else { return None };
            after_delta(condition, delta, value, width)
        }
    }
}

/// `body` with each compare an earlier compare or subtract of its block makes
/// redundant gone, its readers' conditions rewritten.
pub(super) fn compares_reused(body: &LirBody) -> LirBody {
    let flags = flag_effects(body);
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
            let mut at = 0;
            while at < insns.len() {
                if let Some(updates) = redundant(body, block, &insns, at, &flags) {
                    for (reader, name) in updates {
                        let mut new = (*insns[reader]).clone();
                        new.what.as_mut().expect("a conditional reader has semantics").name = Some(name.into());
                        insns[reader] = Arc::new(new);
                    }
                    insns.remove(at);
                    changed = true;
                } else {
                    at += 1;
                }
            }
            block.with_insns(insns)
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The readers (position, new name) of the flags of the compare at `at`, if
/// an earlier flag writer's flags are its own.
fn redundant(
    body: &LirBody,
    block: &crate::model::lir::Block,
    insns: &[Arc<Insn>],
    at: usize,
    flags: &dyn Fn(&Insn) -> Option<(Lanes, Lanes)>,
) -> Option<Vec<(usize, String)>> {
    let what = insns[at].what.as_ref()?;
    if what.op != Operation::Compare || what.name.as_deref() != Some("cmp") || !what.dests.is_empty() {
        return None;
    }
    let compare = operands_of(&insns[at])?;
    // backward: the nearest instruction that writes the flags must be it
    let mut same = None;
    for back in (0..at).rev() {
        let one = &insns[back];
        if one.is_meta() {
            continue;
        }
        let (_, writes) = flags(one)?;
        if writes.is_empty() {
            continue;
        }
        same = matching(compare, operands_of(one)?);
        break;
    }
    let same = same?;
    // forward: every reader before the flags are written again names a
    // condition the earlier flags give the same answer to
    let mut pending = Lanes::read_flags();
    let mut updates = Vec::new();
    for (reader, one) in insns.iter().enumerate().skip(at + 1) {
        if one.is_meta() {
            continue;
        }
        let (reads, writes) = flags(one)?;
        if !reads.is_disjoint(&pending) {
            let name = one.what.as_ref()?.name.as_deref()?;
            let (prefix, condition) = split(name)?;
            let new = replacement(condition, same, compare)?;
            if new != condition {
                updates.push((reader, format!("{prefix}{new}")));
            }
        }
        pending = pending.minus(&writes);
        if pending.is_empty() {
            return Some(updates);
        }
    }
    block.succ.iter().all(|next| successor_overwrites_flags(body, *next, flags, pending)).then_some(updates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Held, Imm, Semantics};
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4, offset: 0 })
    }

    fn constant(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        sources: Vec<Loc>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests: vec![], sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], uses))
    }

    fn cmp(
        at: i64,
        a: Loc,
        b: Loc,
    ) -> Arc<Insn> {
        let uses =
            [&a, &b].iter().filter_map(|loc| if let Loc::Held(h) = loc { Some(h.value) } else { None }).collect();
        insn(at, Operation::Compare, "cmp", vec![a, b], uses)
    }

    fn jcc(
        at: i64,
        name: &str,
    ) -> Arc<Insn> {
        insn(at, Operation::Branch, name, vec![], vec![])
    }

    /// The block, each successor writing the flags before reading them.
    fn body_of(insns: Vec<Arc<Insn>>) -> LirBody {
        let block = LirBlock { succ: vec![1, 2].into(), ..LirBlock::new(0, insns) };
        let next = |at: i64| LirBlock::new(at, vec![cmp(at, held(9), held(9))]);
        let mut body = LirBody::new(
            "f",
            0,
            vec![block.into(), next(1).into(), next(2).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body
    }

    fn names(body: &LirBody) -> Vec<String> {
        body.blocks[0].insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    /// `cmp a,b; jl; cmp a,b; jne`: the second compare is the first's flags.
    #[test]
    fn test_a_repeated_compare_is_the_earlier_one() {
        let body = body_of(vec![cmp(0, held(1), held(2)), jcc(1, "jl"), cmp(2, held(1), held(2)), jcc(3, "jne")]);
        assert_eq!(names(&compares_reused(&body)), ["cmp", "jl", "jne"]);
    }

    /// The pass reaches it: `PeepholeOptimizer` removes the repeated compare
    /// (the other tests call the fold itself).
    #[test]
    fn test_the_peephole_optimizer_removes_a_repeated_compare() {
        let body = body_of(vec![cmp(0, held(1), held(2)), jcc(1, "jl"), cmp(2, held(1), held(2)), jcc(3, "jne")]);
        assert_eq!(names(&super::super::compared(&body)), ["cmp", "jl", "jne"]);
    }

    /// `cmp a,b ... cmp b,a; jg`: the reader reads the swapped condition.
    #[test]
    fn test_a_swapped_compare_is_the_earlier_one_with_the_condition_swapped() {
        let body = body_of(vec![cmp(0, held(1), held(2)), jcc(1, "jl"), cmp(2, held(2), held(1)), jcc(3, "jg")]);
        assert_eq!(names(&compares_reused(&body)), ["cmp", "jl", "jl"]);
    }

    /// A reader with no swapped condition (`js`) keeps the compare.
    #[test]
    fn test_a_swapped_compare_read_by_a_sign_test_stays() {
        let body = body_of(vec![cmp(0, held(1), held(2)), cmp(1, held(2), held(1)), jcc(2, "js")]);
        assert_eq!(names(&compares_reused(&body)), ["cmp", "cmp", "js"]);
    }

    /// A flag writer between them keeps the compare.
    #[test]
    fn test_a_flag_writer_between_keeps_the_second_compare() {
        let add = insn(1, Operation::Binary, "add", vec![held(3), held(4)], vec![3, 4]);
        let body = body_of(vec![cmp(0, held(1), held(2)), add, cmp(2, held(1), held(2)), jcc(3, "jne")]);
        assert_eq!(names(&compares_reused(&body)), ["cmp", "add", "cmp", "jne"]);
    }

    /// The eight rows of the pin's `ImmDelta` table: the compare's constant is
    /// one off the earlier compare's.
    #[test]
    fn test_a_constant_one_off_the_earlier_one_rewrites_the_condition() {
        // (earlier constant, compare constant, reader, rewritten reader)
        for (earlier, later, reader, rewritten) in [
            (5, 6, "jl", "jle"),
            (5, 6, "jb", "jbe"),
            (5, 6, "jge", "jg"),
            (5, 6, "jae", "ja"),
            (5, 4, "jg", "jge"),
            (5, 4, "ja", "jae"),
            (5, 4, "jle", "jl"),
            (5, 4, "jbe", "jb"),
        ] {
            let body = body_of(vec![
                cmp(0, held(1), constant(earlier)),
                jcc(1, "jne"),
                cmp(2, held(1), constant(later)),
                jcc(3, reader),
            ]);
            assert_eq!(names(&compares_reused(&body)), ["cmp", "jne", rewritten], "{reader} at {later}");
        }
    }

    /// A reader the table has no row for (`jle` at C+1) keeps the compare.
    #[test]
    fn test_a_constant_one_off_with_the_wrong_reader_stays() {
        let body = body_of(vec![cmp(0, held(1), constant(5)), cmp(1, held(1), constant(6)), jcc(2, "jle")]);
        assert_eq!(names(&compares_reused(&body)), ["cmp", "cmp", "jle"]);
    }

    /// `x < 0` after `x < 1` is `x <= 0` unsigned only if the constant is not
    /// the minimum: `jb` of 0 has no `jbe` form.
    #[test]
    fn test_an_unsigned_below_zero_keeps_its_compare() {
        let body = body_of(vec![cmp(0, held(1), constant(-1)), cmp(1, held(1), constant(0)), jcc(2, "jb")]);
        assert_eq!(names(&compares_reused(&body)), ["cmp", "cmp", "jb"]);
    }

    /// The work is linear in the instructions: a block of `n` compare pairs
    /// asks the flag effects of each instruction once (kept on it), so twice
    /// the pairs is about twice the effects worked out, not four times.
    #[test]
    fn test_the_scan_is_linear_in_the_instructions() {
        let worked = |pairs: i64| {
            let mut insns = Vec::new();
            for pair in 0..pairs {
                let at = pair * 4;
                let (a, b) = ((pair * 2) as u32 + 1, (pair * 2) as u32 + 2);
                insns.extend([
                    cmp(at, held(a), held(b)),
                    jcc(at + 1, "jl"),
                    cmp(at + 2, held(a), held(b)),
                    jcc(at + 3, "jne"),
                ]);
            }
            let body = body_of(insns);
            let before = crate::backend::liveness::effects_worked_out();
            let done = compares_reused(&body);
            assert_eq!(done.blocks[0].insns.len() as i64, pairs * 3, "each repeated compare goes");
            crate::backend::liveness::effects_worked_out() - before
        };
        let (small, large) = (worked(300), worked(600));
        assert!(large <= small * 2 + 8, "{small} effects for 300 pairs, {large} for 600");
    }
}
