//! A subtract whose difference nothing reads is a compare.
//!
//! Translated from the first switch of `X86InstrInfo::optimizeCompareInstr`
//! ([X86InstrInfo.cpp#L5477](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5477)),
//! which the pin's PeepholeOptimizer runs before allocation on every
//! `SUB` it can analyse as a compare.
//!
//! # How it works
//! `EmitCmp` builds `X86ISD::SUB` for a compare, "to enable CSE between SUB and
//! CMP". After MachineCSE and MachineSink, a `SUB` (register, immediate or
//! memory second operand) with no reader of its result writes only the flags,
//! which is what `CMP` does: the destination goes and the opcode becomes the
//! compare's. Left as a `SUB`, the dead result gets a register, and a copy
//! to keep the operand it overwrites.
//!
//! ```text
//!  d := sub a, b      (nothing reads d)         cmp a, b
//!  jl                                           jl
//! ```
//!
//! A compare of a value with zero (`test x, x`, `cmp x, 0`) goes where an
//! earlier instruction of the block that made `x` set the flags it reads:
//!
//! ```text
//!  x := shr y, 2                      x := shr y, 2
//!  test x, x           =>             jne L
//!  jne L
//! ```
//!
//! The instruction must be one `isDefConvertible` lists (here `add`, `sub`,
//! `inc`, `dec`, `neg`, `and`, `or`, `xor` and `shl`/`shr`/`sar` by a count
//! that is not zero), no instruction between it and the compare may write the
//! flags, and every reader of the flags after the compare must read only what
//! the instruction sets as a compare of zero does: ZF always, SF unless
//! `NoSignFlag`, OF only where the instruction clears it (`and`, `or`, `xor`);
//! never CF. The flags must not live out of the block.
//!
//! Not translated: a compare redundant with another compare or subtract
//! (`isRedundantFlagInstr`, `IsSwapped`, `ImmDelta`), `isUseDefConvertible`
//! (`popcnt`), `adc`/`sbb`/`shld`/`shrd`/`lzcnt`/`tzcnt` and the BMI group as
//! the flag source, the backward scan into a single predecessor, `MOV32r0`
//! motion and the NF variants (APX).
//!
//! # Runs
//! A machine phase on SSA after MachineSink and ahead of MachineCombiner (the
//! pin's `addMachineSSAOptimization` order), at every level but -O0.

use std::sync::Arc;

use iced_x86::RflagsBits;

use crate::backend::lanes::Lanes;
use crate::backend::precse::flag_lanes;
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};

pub struct PeepholeOptimizer;

impl LIRTransform for PeepholeOptimizer {
    fn class_name(&self) -> &'static str {
        "PeepholeOptimizer"
    }

    fn name(&self) -> &str {
        "peepholeopt"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(compared(&body))
    }
}

/// `body` with the compares an earlier instruction's flags make redundant gone,
/// and each dead-result subtract a compare.
pub fn compared(body: &LirBody) -> LirBody {
    flags_reused(&subtracts_compared(body))
}

/// `body` with each dead-result subtract a compare.
fn subtracts_compared(body: &LirBody) -> LirBody {
    let mut read: HashSet<u32> = HashSet::default();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            read.extend(phi.incoming.iter().map(|&(_, value)| value));
        }
        for one in &block.insns {
            read.extend(one.uses.iter().copied());
        }
    }
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            if !block.insns.iter().any(|one| dead_subtract(one, &read).is_some()) {
                return block.clone();
            }
            changed = true;
            let insns = block
                .insns
                .iter()
                .map(|one| match dead_subtract(one, &read) {
                    Some(result) => Arc::new(compare_of(one, result)),
                    None => Arc::clone(one),
                })
                .collect::<Vec<_>>();
            block.with_insns(insns)
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The value `one` subtracts into, where that is a register of its own that
/// nothing reads and `one` is a plain `sub`.
fn dead_subtract(
    one: &Insn,
    read: &HashSet<u32>,
) -> Option<u32> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Binary
        || what.name.as_deref() != Some("sub")
        || what.indirect
        || one.group.is_some()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
    {
        return None;
    }
    let [Loc::Held(result)] = what.dests[..] else { return None };
    let [_, Loc::Held(_) | Loc::Imm(_) | Loc::Mem(_)] = what.sources[..] else { return None };
    (one.defines == [result.value] && !read.contains(&result.value)).then_some(result.value)
}

/// `one` as the compare of its operands.
fn compare_of(
    one: &Insn,
    result: u32,
) -> Insn {
    let what = one.what.as_ref().expect("a subtract");
    let compare = Semantics { name: Some("cmp".into()), dests: Vec::new(), ..what.clone() };
    Insn {
        what: Some(Semantics { op: Operation::Compare, ..compare }),
        defines: Vec::new(),
        widths: one.widths.iter().copied().filter(|&(value, _)| value != result).collect(),
        ..one.clone()
    }
}

/// What the instruction that made a value tells a compare of it with zero.
#[derive(Clone, Copy)]
struct Sets {
    /// `OF` is clear, as a compare of zero leaves it (`and`, `or`, `xor`).
    clears_overflow: bool,
}

/// `isDefConvertible`: what `one` sets if its flags are those of comparing its
/// result with zero.
fn defines_flags(
    one: &Insn,
    reads: &HashMap<u32, usize>,
) -> Option<(u32, Sets)> {
    let what = one.what.as_ref()?;
    let [Loc::Held(result)] = what.dests[..] else { return None };
    if one.defines != [result.value] || one.group.is_some() || what.indirect {
        return None;
    }
    let name = what.name.as_deref()?;
    let sets = |clears_overflow| Some((result.value, Sets { clears_overflow }));
    match (what.op, name, &what.sources[..]) {
        (Operation::Binary, "add" | "sub", [_, Loc::Held(_) | Loc::Imm(_) | Loc::Mem(_)]) => sets(false),
        (Operation::Binary, "and" | "or" | "xor", [_, Loc::Held(_) | Loc::Imm(_) | Loc::Mem(_)]) => sets(true),
        (Operation::Unary, "inc" | "dec" | "neg", [_]) => sets(false),
        // a shift sets ZF and SF only for a count that is not zero (masked to
        // five bits); `shl` by one to three may become a `lea` unless the
        // shifted value is read elsewhere (`isTruncatedShiftCountForLEA`)
        (Operation::Binary, "shr" | "sar" | "shl", [Loc::Held(source), Loc::Imm(count)]) if count.address.is_none() => {
            let count = count.value & 31;
            if count == 0 || (name == "shl" && (1..=3).contains(&count) && reads.get(&source.value) != Some(&1)) {
                return None;
            }
            sets(false)
        }
        _ => None,
    }
}

/// The part of a condition name after the `j`, `set` or `cmov`.
fn condition_of(name: &str) -> Option<&str> {
    ["cmov", "set", "j"].iter().find_map(|prefix| name.strip_prefix(prefix))
}

/// Whether a reader of the flags named `name` reads only what `sets` leaves as
/// a compare of zero does.
fn reads_what_it_sets(
    name: &str,
    sets: Sets,
) -> bool {
    match condition_of(name) {
        Some("e" | "z" | "ne" | "nz" | "s" | "ns") => true,
        Some("g" | "ge" | "l" | "le" | "nl" | "nle" | "ng" | "nge" | "o" | "no") => sets.clears_overflow,
        _ => false,
    }
}

/// `body` with each compare of a value with zero gone where the instruction
/// that made the value in the same block set the flags it reads.
fn flags_reused(body: &LirBody) -> LirBody {
    let mut reads: HashMap<u32, usize> = HashMap::default();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            for &(_, value) in &phi.incoming {
                *reads.entry(value).or_default() += 1;
            }
        }
        for one in &block.insns {
            for &value in &one.uses {
                *reads.entry(value).or_default() += 1;
            }
        }
    }
    // The flag lanes `one` reads and writes.
    let flags = |one: &Insn| -> Option<(Lanes, Lanes)> {
        let Some(effect) = crate::backend::liveness::effect(body.regs(), body.bits, one) else {
            // a conditional branch reads the flags and writes nothing
            let branch = one.what.as_ref().filter(|what| what.op == Operation::Branch && !what.indirect);
            return branch
                .filter(|what| what.name.as_deref().and_then(condition_of).is_some())
                .map(|_| (Lanes::flags(u32::MAX), Lanes::new()));
        };
        Some((flag_lanes(&effect.reads), flag_lanes(&effect.writes)))
    };
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
            let mut gone: Vec<usize> = Vec::new();
            for at in 0..insns.len() {
                if let Some(value) = compared_with_zero(&insns[at])
                    && reusable(body, block, &insns, at, value, &reads, &flags)
                {
                    gone.push(at);
                }
            }
            if gone.is_empty() {
                return block.clone();
            }
            changed = true;
            block.with_insns(
                insns
                    .iter()
                    .enumerate()
                    .filter(|(at, _)| !gone.contains(at))
                    .map(|(_, one)| Arc::clone(one))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The value `test x, x` or `cmp x, 0` compares with zero.
fn compared_with_zero(one: &Insn) -> Option<u32> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Compare || one.group.is_some() || !what.dests.is_empty() {
        return None;
    }
    match (what.name.as_deref()?, &what.sources[..]) {
        ("test", [Loc::Held(a), Loc::Held(b)]) if a.value == b.value && a.width == b.width => Some(a.value),
        ("cmp", [Loc::Held(a), Loc::Imm(zero)]) if zero.value == 0 && zero.address.is_none() => Some(a.value),
        _ => None,
    }
}

/// Whether the compare at `at` of `value` with zero is redundant: the flags of
/// the instruction that made `value` reach every reader unchanged.
fn reusable(
    body: &LirBody,
    block: &crate::model::lir::Block,
    insns: &[Arc<Insn>],
    at: usize,
    value: u32,
    reads: &HashMap<u32, usize>,
    flags: &dyn Fn(&Insn) -> Option<(Lanes, Lanes)>,
) -> bool {
    // the scan back from the compare: no other writer of the flags before the
    // definition of the value
    let mut defined = None;
    for back in (0..at).rev() {
        let one = &insns[back];
        if one.defines.contains(&value) {
            defined = Some(back);
            break;
        }
        if one.is_meta() {
            continue;
        }
        match flags(one) {
            Some((_, writes)) if writes.is_empty() => {}
            _ => return false,
        }
    }
    let Some(def) = defined else { return false };
    let Some((_, sets)) = defines_flags(&insns[def], reads) else { return false };
    let width = |loc: &Loc| match loc {
        Loc::Held(held) => Some(held.width),
        _ => None,
    };
    let compared = insns[at].what.as_ref().and_then(|what| what.sources.first()).and_then(width);
    if compared != insns[def].what.as_ref().and_then(|what| what.dests.first()).and_then(width) {
        return false;
    }
    // the scan forward from the compare: the readers of the flags read only
    // what the definition sets, until something writes them again. A write of
    // some flags (`inc` leaves CF) kills only those.
    let mut pending = Lanes::flags(
        (RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF) as u32,
    );
    for one in &insns[at + 1..] {
        if one.is_meta() {
            continue;
        }
        let Some((reads, writes)) = flags(one) else { return false };
        if !reads.is_disjoint(&pending) {
            let named = one.what.as_ref().and_then(|what| what.name.as_deref());
            if !named.is_some_and(|name| reads_what_it_sets(name, sets)) {
                return false;
            }
        }
        pending = pending.minus(&writes);
        if pending.is_empty() {
            return true;
        }
    }
    // not written again in the block: the flags must not be read in a successor
    // before they are written there
    block.succ.iter().all(|next| successor_overwrites_flags(body, *next, flags, pending))
}

/// Whether the block at `at` writes every one of `pending` before it reads any.
fn successor_overwrites_flags(
    body: &LirBody,
    at: i64,
    flags: &dyn Fn(&Insn) -> Option<(Lanes, Lanes)>,
    mut pending: Lanes,
) -> bool {
    let Some(block) = body.blocks.iter().find(|one| one.at == at) else { return false };
    for one in &block.insns {
        if one.is_meta() {
            continue;
        }
        let Some((reads, writes)) = flags(one) else { return false };
        if !reads.is_disjoint(&pending) {
            return false;
        }
        pending = pending.minus(&writes);
        if pending.is_empty() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Held, Imm};
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
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

    fn names(
        body: &LirBody,
        kept: bool,
    ) -> Vec<String> {
        let _ = kept;
        body.blocks[0].insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    fn body_of(insns: Vec<Arc<Insn>>) -> LirBody {
        let block = LirBlock { succ: vec![1, 2].into(), ..LirBlock::new(0, insns) };
        // each successor writes the flags before it reads them
        let next = |at: i64| {
            LirBlock::new(
                at,
                vec![insn(at, Operation::Compare, "cmp", vec![], vec![held(9), held(9)], vec![], vec![9, 9])],
            )
        };
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

    /// `v3 := sub v1, v2; jl` with nothing reading `v3` (a compare `EmitCmp`
    /// built as a subtract): the dead result took a register and a copy of the
    /// operand it overwrote (x_heapsort: 12 `sub` where the pin has 1, +16
    /// `mov`).
    #[test]
    fn test_a_subtract_nothing_reads_is_a_compare() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "sub", vec![held(3)], vec![held(1), held(2)], vec![3], vec![1, 2]),
            insn(1, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        let done = compared(&body);
        assert_eq!(names(&done, true), ["cmp", "jl"]);
        let cmp = &done.blocks[0].insns[0];
        assert!(cmp.defines.is_empty() && cmp.uses == [1, 2]);
        let what = cmp.what.as_ref().expect("a compare");
        assert!(what.op == Operation::Compare && what.dests.is_empty() && what.sources == [held(1), held(2)]);
    }

    /// The immediate form is a compare too.
    #[test]
    fn test_a_subtract_of_an_immediate_nothing_reads_is_a_compare() {
        let imm = Loc::Imm(Imm { value: 7, width: 4, address: None });
        let body = body_of(vec![
            insn(0, Operation::Binary, "sub", vec![held(3)], vec![held(1), imm], vec![3], vec![1]),
            insn(1, Operation::Branch, "jae", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["cmp", "jae"]);
    }

    /// A difference somebody reads stays a subtract.
    #[test]
    fn test_a_subtract_something_reads_stays() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "sub", vec![held(3)], vec![held(1), held(2)], vec![3], vec![1, 2]),
            insn(1, Operation::Move, "mov", vec![held(4)], vec![held(3)], vec![4], vec![3]),
            insn(2, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), false), ["sub", "mov", "jl"]);
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 1, address: None })
    }

    /// `v2 := shr v1, 2; test v2, v2; jne`, the loop test of x_isqrt: the shift
    /// set ZF, the pin deletes the test.
    #[test]
    fn test_a_test_of_a_shifted_value_is_the_shifts_flags() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "shr", vec![held(2)], vec![held(1), imm(2)], vec![2], vec![1]),
            insn(1, Operation::Compare, "test", vec![], vec![held(2), held(2)], vec![], vec![2, 2]),
            insn(2, Operation::Branch, "jne", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["shr", "jne"]);
    }

    /// `cmp v2, 0` is the same compare.
    #[test]
    fn test_a_compare_with_zero_of_an_anded_value_is_the_ands_flags() {
        let zero = Loc::Imm(Imm { value: 0, width: 4, address: None });
        let body = body_of(vec![
            insn(0, Operation::Binary, "and", vec![held(2)], vec![held(1), held(3)], vec![2], vec![1, 3]),
            insn(1, Operation::Compare, "cmp", vec![], vec![held(2), zero], vec![], vec![2]),
            insn(2, Operation::Branch, "jg", vec![], vec![], vec![], vec![]),
        ]);
        // `and` clears OF: a signed condition reads what a compare of zero
        // gives
        assert_eq!(names(&compared(&body), true), ["and", "jg"]);
    }

    /// `add` leaves OF as an addition does, not as a compare of zero: `jg`
    /// reads it.
    #[test]
    fn test_a_signed_branch_after_an_add_keeps_its_test() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "add", vec![held(2)], vec![held(1), held(3)], vec![2], vec![1, 3]),
            insn(1, Operation::Compare, "test", vec![], vec![held(2), held(2)], vec![], vec![2, 2]),
            insn(2, Operation::Branch, "jg", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["add", "test", "jg"]);
        // ZF alone is the same for both
        let body = body_of(vec![
            insn(0, Operation::Binary, "add", vec![held(2)], vec![held(1), held(3)], vec![2], vec![1, 3]),
            insn(1, Operation::Compare, "test", vec![], vec![held(2), held(2)], vec![], vec![2, 2]),
            insn(2, Operation::Branch, "je", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["add", "je"]);
    }

    /// CF is not a compare of zero's: `ja` after a shift keeps the test.
    #[test]
    fn test_an_unsigned_branch_keeps_its_test() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "shr", vec![held(2)], vec![held(1), imm(2)], vec![2], vec![1]),
            insn(1, Operation::Compare, "test", vec![], vec![held(2), held(2)], vec![], vec![2, 2]),
            insn(2, Operation::Branch, "ja", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["shr", "test", "ja"]);
    }

    /// A flag writer between the definition and the compare, or a shift by
    /// zero: the flags are not the value's.
    #[test]
    fn test_a_flag_writer_between_keeps_the_test() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "shr", vec![held(2)], vec![held(1), imm(2)], vec![2], vec![1]),
            insn(1, Operation::Binary, "add", vec![held(4)], vec![held(3), held(3)], vec![4], vec![3, 3]),
            insn(2, Operation::Compare, "test", vec![], vec![held(2), held(2)], vec![], vec![2, 2]),
            insn(3, Operation::Branch, "jne", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["shr", "add", "test", "jne"]);
        let body = body_of(vec![
            insn(0, Operation::Binary, "shr", vec![held(2)], vec![held(1), imm(32)], vec![2], vec![1]),
            insn(1, Operation::Compare, "test", vec![], vec![held(2), held(2)], vec![], vec![2, 2]),
            insn(2, Operation::Branch, "jne", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["shr", "test", "jne"], "a count of 32 is zero");
    }

    fn narrow(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
    }

    /// `v2 := shr v1, 2` (a dword) read as a word by `test v2w, v2w`: the
    /// shift's ZF and SF are the dword's, the test's are the word's, and the
    /// test went.
    #[test]
    fn test_a_word_test_of_a_dword_shift_keeps_its_test() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "shr", vec![held(2)], vec![held(1), imm(2)], vec![2], vec![1]),
            insn(1, Operation::Compare, "test", vec![], vec![narrow(2, 2), narrow(2, 2)], vec![], vec![2, 2]),
            insn(2, Operation::Branch, "jne", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["shr", "test", "jne"]);
    }

    /// The other way: a word shift read as a dword by `cmp v2, 0` (the upper
    /// half is stale).
    #[test]
    fn test_a_dword_compare_of_a_word_shift_keeps_its_compare() {
        let zero = Loc::Imm(Imm { value: 0, width: 4, address: None });
        let body = body_of(vec![
            insn(0, Operation::Binary, "shr", vec![narrow(2, 2)], vec![narrow(1, 2), imm(2)], vec![2], vec![1]),
            insn(1, Operation::Compare, "cmp", vec![], vec![held(2), zero], vec![], vec![2]),
            insn(2, Operation::Branch, "jne", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["shr", "cmp", "jne"]);
    }

    /// A word subtract nothing reads is a word compare, not a dword one.
    #[test]
    fn test_a_word_subtract_is_a_word_compare() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "sub", vec![narrow(3, 2)], vec![narrow(1, 2), narrow(2, 2)], vec![3], vec![1, 2]),
            insn(1, Operation::Branch, "ja", vec![], vec![], vec![], vec![]),
        ]);
        let done = compared(&body);
        assert_eq!(names(&done, true), ["cmp", "ja"]);
        assert_eq!(done.blocks[0].insns[0].what.as_ref().unwrap().sources, [narrow(1, 2), narrow(2, 2)]);
    }

    /// `shr; test; inc; jb`: `inc` leaves CF alone, so `jb` reads the carry of
    /// the `test` (clear), not of the shift. The test went because `inc`
    /// "wrote the flags".
    #[test]
    fn test_a_carry_read_after_an_inc_keeps_the_test() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "shr", vec![held(2)], vec![held(1), imm(2)], vec![2], vec![1]),
            insn(1, Operation::Compare, "test", vec![], vec![held(2), held(2)], vec![], vec![2, 2]),
            insn(2, Operation::Unary, "inc", vec![held(4)], vec![held(3)], vec![4], vec![3]),
            insn(3, Operation::Branch, "jb", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["shr", "test", "inc", "jb"]);
    }
}
