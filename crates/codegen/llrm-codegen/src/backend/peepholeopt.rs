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
//! # Example
//! A subtract whose result nothing reads, ahead of a branch on the flags:
//!
//! ```text
//!  before                  after
//!  d := sub a, b           cmp a, b
//!  jl L                    jl L
//! ```
//!
//! From unit test `test_a_subtract_nothing_reads_is_a_compare` in
//! `peepholeopt.rs`, which asserts the `sub` becomes a `cmp` with no
//! destination.
//!
//! # Runs
//! A machine phase on SSA after MachineSink and ahead of MachineCombiner (the
//! pin's `addMachineSSAOptimization` order), at every level but -O0.

use std::sync::Arc;

use crate::backend::lanes::Lanes;
use crate::backend::precse::flag_lanes;
use crate::model::ir::{Imm, Loc, Operation, Semantics};
use crate::model::lir::{self, Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};

/// `size`: the function is built for size, where an immediate is not folded
/// into more than one reader (it is longer than the register).
pub struct PeepholeOptimizer {
    pub size: bool,
}

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
        Ok(loads_folded(&compared(&immediates_folded(&recurrences_commuted(&body), self.size))))
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
pub(crate) fn condition_of(name: &str) -> Option<&str> {
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
    let mut pending = Lanes::read_flags();
    for one in &insns[at + 1..] {
        if one.is_meta() {
            continue;
        }
        let Some((reads, writes)) = flags(one) else {
            return false;
        };
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
    // not written again in the block: a successor must write them before it
    // reads them
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

/// `PeepholeOptimizer`'s `foldImmediate` (`X86InstrInfo::foldImmediateImpl`,
/// [L6012](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L6012)):
/// a register an earlier `mov` of the same block set to a constant, read as
/// the operand of an `add`, `sub`, `and`, `or` or `xor`, is that constant in
/// the immediate form. The first operand that is such a register is the one
/// folded; a commutative instruction swaps it to the second place, `sub` takes
/// the constant only as its second operand. The `mov` goes when nothing else
/// reads it. Built for size, a constant several instructions read stays a
/// register.
///
/// Not translated: the `COPY` and shift forms and `cmp`/`test`/`adc`/`sbb`.
pub fn immediates_folded(
    body: &LirBody,
    size: bool,
) -> LirBody {
    let mut reads: HashMap<u32, usize> = HashMap::default();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            for &(_from, value) in &phi.incoming {
                *reads.entry(value).or_default() += 1;
            }
        }
        for one in &block.insns {
            for value in one.uses.iter().copied().chain(one.requires.iter().map(|(held, _)| held.value)) {
                *reads.entry(value).or_default() += 1;
            }
        }
    }
    // A value with no definition is an undefined read (`ProcessImplicitDefs`
    // removed its `IMPLICIT_DEF`): swapping it into the tied first place
    // would make the instruction read a register nothing wrote.
    let mut defined: HashSet<u32> = HashSet::default();
    for block in &body.blocks {
        defined.extend(block.phis.iter().map(|phi| phi.result));
        for one in &block.insns {
            defined.extend(one.defines.iter().copied().chain(one.delivers.iter().map(|(held, _)| held.value)));
        }
    }
    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut constants: HashMap<u32, (usize, Imm)> = HashMap::default();
        let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
        let mut changed = false;
        for at in 0..insns.len() {
            if let Some((value, imm)) = constant_move(&insns[at]) {
                constants.insert(value, (at, imm));
                continue;
            }
            let Some((folded, value)) = copy_folded(&insns[at], &insns[at + 1..], &constants, &reads, size)
                .or_else(|| folded_use(&insns[at], &constants, &reads, &defined, size))
            else {
                continue;
            };
            insns[at] = Arc::new(folded);
            changed = true;
            let count = reads.entry(value).or_default();
            *count = count.saturating_sub(1);
            if *count == 0 {
                let (def, _) = constants[&value];
                insns[def] = lir::bytes_only(&insns[def]);
                constants.remove(&value);
            }
        }
        if changed {
            block.insns = insns.into();
        }
    }
    out
}

/// The register and constant of a plain `mov reg, imm`.
fn constant_move(one: &Insn) -> Option<(u32, Imm)> {
    let what = one.what.as_ref()?;
    let ([Loc::Held(dest)], [Loc::Imm(imm)]) = (&what.dests[..], &what.sources[..]) else { return None };
    (what.op == Operation::Move
        && what.name.as_deref() == Some("mov")
        && imm.address.is_none()
        && one.defines == [dest.value]
        && one.group.is_none()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty())
    .then_some((dest.value, imm.clone()))
}

/// `foldImmediateImpl`'s `COPY` case: `d := mov c` of a register a `mov` set to
/// a constant is a `mov` of the constant at the copy, where the register-class
/// allows it (a 32-bit or 8-bit register, as the pin's `MOV32ri` and `MOV8ri`).
/// A zero is `MOV32r0`, which clobbers the flags, so only where the flags are
/// written before they are read (`flags_dead`).
fn copy_folded(
    one: &Insn,
    after: &[Arc<Insn>],
    constants: &HashMap<u32, (usize, Imm)>,
    reads: &HashMap<u32, usize>,
    size: bool,
) -> Option<(Insn, u32)> {
    let what = one.what.as_ref()?;
    let ([Loc::Held(dest)], [Loc::Held(source)]) = (&what.dests[..], &what.sources[..]) else { return None };
    if what.op != Operation::Move
        || what.name.as_deref() != Some("mov")
        || one.defines != [dest.value]
        || one.group.is_some()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.clobbers.is_empty()
        || ![1, 4].contains(&dest.width)
        || dest.width != source.width
    {
        return None;
    }
    let (_, imm) = constants.get(&source.value)?;
    if (size && reads.get(&source.value).copied().unwrap_or(0) > 1) || (imm.value == 0 && !flags_dead(after)) {
        return None;
    }
    let mut folded = one.clone();
    folded.what = Some(Semantics { sources: vec![Loc::Imm(Imm { width: dest.width, ..imm.clone() })], ..what.clone() });
    folded.uses.retain(|&used| used != source.value);
    folded.widths.retain(|&(used, _)| used != source.value);
    Some((folded, source.value))
}

/// `computeRegisterLiveness(EFLAGS, UseMI) == LQR_Dead`: the flags are written
/// before they are read, within the ten instructions after the copy. A block
/// that ends first leaves them unknown, as the pin finds them.
fn flags_dead(after: &[Arc<Insn>]) -> bool {
    for one in after.iter().filter(|one| !one.is_meta()).take(10) {
        let Some(what) = one.what.as_ref() else { continue };
        let name = what.name.as_deref().unwrap_or("");
        if (what.op == Operation::Branch && name != "jmp")
            || ["cmov", "set", "adc", "sbb"].iter().any(|prefix| name.starts_with(prefix))
        {
            return false;
        }
        if matches!(
            what.op,
            Operation::Binary | Operation::Compare | Operation::Unary
        )
            && ["add", "sub", "and", "or", "xor", "cmp", "test", "inc", "dec", "neg", "shl", "shr", "sar"]
                .contains(&name)
        {
            return true;
        }
    }
    false
}

/// `one` with its first constant operand an immediate, and the register that
/// was.
fn folded_use(
    one: &Insn,
    constants: &HashMap<u32, (usize, Imm)>,
    reads: &HashMap<u32, usize>,
    defined: &HashSet<u32>,
    size: bool,
) -> Option<(Insn, u32)> {
    let what = one.what.as_ref()?;
    let commutes = match (what.op, what.name.as_deref()?) {
        (Operation::Binary, "add" | "and" | "or" | "xor") => true,
        (Operation::Binary, "sub") => false,
        _ => return None,
    };
    let ([Loc::Held(_)], [Loc::Held(first), Loc::Held(second)]) = (&what.dests[..], &what.sources[..]) else {
        return None;
    };
    if what.indirect
        || one.group.is_some()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || first.value == second.value
    {
        return None;
    }
    let (kept, constant, swapped) = if let Some((_, imm)) = constants.get(&first.value).filter(|_| commutes) {
        (*second, (first.value, imm.clone()), true)
    } else if let Some((_, imm)) = constants.get(&second.value) {
        (*first, (second.value, imm.clone()), false)
    } else {
        return None;
    };
    let (value, imm) = constant;
    if (size && reads.get(&value).copied().unwrap_or(0) > 1) || imm.width != kept.width && imm.width != second.width {
        return None;
    }
    if swapped && !defined.contains(&kept.value) {
        return None;
    }
    // A 64-bit instruction's immediate is 32 bits, sign-extended
    // (`foldImmediateImpl`: `isInt<32>`): `0xFFFFFFF7` is not -9 there.
    if kept.width == 8 && i32::try_from(imm.value).is_err() {
        return None;
    }
    let sources = vec![Loc::Held(kept), Loc::Imm(Imm { width: kept.width, ..imm })];
    let mut folded = one.clone();
    folded.what = Some(Semantics { sources, ..what.clone() });
    folded.uses.retain(|&used| used != value);
    folded.widths.retain(|&(used, _)| used != value);
    Some((folded, value))
}

/// `body` with each single-use load folded into the instruction that reads it
/// (`isLoadFoldable`, `foldLoadInto`, `X86InstrInfo::optimizeLoadInstr`): the
/// block's `d := mov [m]` ahead of `cmp d, r`, `cmp r, d` or
/// `add|sub|and|or|xor r, d` is the same instruction with `[m]` as the operand.
/// A store, a call or anything else `isLoadFoldBarrier` drops the loads waiting
/// to fold.
///
/// Not translated: the other instructions `foldMemoryOperand` knows (`imul`,
/// the extends, the x87 forms), a load whose result is read twice.
pub fn loads_folded(body: &LirBody) -> LirBody {
    let mut reads: HashMap<u32, usize> = HashMap::default();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            for &(_from, value) in &phi.incoming {
                *reads.entry(value).or_default() += 1;
            }
        }
        for one in &block.insns {
            // Readers, not operands: `test v, v` is one reader of `v`
            // (`hasOneNonDBGUser`).
            let mut seen: Vec<u32> =
                one.uses.iter().copied().chain(one.requires.iter().map(|(held, _)| held.value)).collect();
            seen.sort_unstable();
            seen.dedup();
            for value in seen {
                *reads.entry(value).or_default() += 1;
            }
        }
    }
    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut loads: HashMap<u32, usize> = HashMap::default();
        let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
        let mut changed = false;
        for at in 0..insns.len() {
            if let Some(value) = foldable_load(&insns[at]).filter(|value| reads.get(value) == Some(&1)) {
                loads.insert(value, at);
                continue;
            }
            if !loads.is_empty() {
                if let Some((folded, value)) = load_folded(&insns[at], &loads, &insns) {
                    let def = loads.remove(&value).expect("a waiting load");
                    insns[def] = lir::bytes_only(&insns[def]);
                    insns[at] = Arc::new(folded);
                    changed = true;
                }
                if crate::backend::machinelicm::keeps_loads(&insns[at]) {
                    loads.clear();
                }
            }
        }
        if changed {
            block.insns = insns.into();
        }
    }
    out
}

/// The value a plain load defines, where nothing but its address is read.
fn foldable_load(one: &Insn) -> Option<u32> {
    let what = one.what.as_ref()?;
    let ([Loc::Held(dest)], [Loc::Mem(cell)]) = (&what.dests[..], &what.sources[..]) else { return None };
    (what.op == Operation::Move
        && what.name.as_deref() == Some("mov")
        && one.defines == [dest.value]
        && dest.width == cell.width
        && !one.volatile()
        && one.group.is_none()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty())
    .then_some(dest.value)
}

/// `one` with the memory a waiting load reads as its operand, and the load's
/// value.
fn load_folded(
    one: &Insn,
    loads: &HashMap<u32, usize>,
    insns: &[Arc<Insn>],
) -> Option<(Insn, u32)> {
    let what = one.what.as_ref()?;
    let name = what.name.as_deref()?;
    // `test r, r` of a waiting load is `cmp [m], 0` (`foldMemoryOperandImpl`:
    // TEST_rr with both operands the register becomes CMP_mi of zero).
    if (what.op, name) == (Operation::Compare, "test")
        && one.group.is_none()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && !what.indirect
        && let [Loc::Held(first), Loc::Held(second)] = &what.sources[..]
        && first.value == second.value
        && loads.contains_key(&first.value)
        && let Some(Loc::Mem(cell)) = insns[loads[&first.value]].what.as_ref().and_then(|w| w.sources.first())
        && first.width == cell.width
    {
        let load = first.value;
        let def = &insns[loads[&load]];
        let zero = Loc::Imm(Imm { value: 0, width: cell.width, address: None });
        let mut folded = one.clone();
        folded.what =
            Some(Semantics { name: Some("cmp".into()), sources: vec![Loc::Mem(cell.clone()), zero], ..what.clone() });
        folded.uses.retain(|&used| used != load);
        folded.uses.extend(def.uses.iter().copied());
        folded.widths.retain(|&(used, _)| used != load);
        folded.widths.extend(def.widths.iter().copied());
        return Some((folded, load));
    }
    let commutes = match (what.op, name) {
        (Operation::Compare, "cmp") => false,
        (Operation::Binary, "add" | "and" | "or" | "xor") => true,
        (Operation::Binary, "sub") => false,
        _ => return None,
    };
    if one.group.is_some() || !one.requires.is_empty() || !one.delivers.is_empty() || what.indirect {
        return None;
    }
    let ([.., Loc::Held(_)] | [], [Loc::Held(first), Loc::Held(second)]) = (&what.dests[..], &what.sources[..]) else {
        return None;
    };
    let compare = what.op == Operation::Compare;
    // the memory goes where the pin's `rm` or `mr` form has it: second for an
    // arithmetic instruction (the first is tied to the result), either for a
    // compare, and first for a commutative one whose second is not the load
    let (kept, load, swapped) = if loads.contains_key(&second.value) && first.value != second.value {
        (*first, second.value, false)
    } else if (compare || commutes) && loads.contains_key(&first.value) && first.value != second.value {
        (*second, first.value, !compare)
    } else {
        return None;
    };
    let Some(Loc::Mem(cell)) = insns[loads[&load]].what.as_ref().and_then(|w| w.sources.first()) else { return None };
    // The memory operand has the instruction's width: a word read as its low
    // byte, or a byte as a word, is not the same instruction.
    let read = if load == second.value { second } else { first };
    if read.width != cell.width || kept.width != cell.width {
        return None;
    }
    let held = if swapped {
        [Loc::Held(kept), Loc::Mem(cell.clone())]
    } else if load == second.value {
        [Loc::Held(kept), Loc::Mem(cell.clone())]
    } else {
        [Loc::Mem(cell.clone()), Loc::Held(kept)]
    };
    let def = &insns[loads[&load]];
    let mut folded = one.clone();
    folded.what = Some(Semantics { sources: held.to_vec(), ..what.clone() });
    let at = folded.uses.iter().position(|&used| used == load)?;
    folded.uses.splice(at..=at, def.uses.iter().copied());
    folded.widths.retain(|&(used, _)| used != load);
    folded.widths.extend(def.widths.iter().copied());
    Some((folded, load))
}

/// `MaxRecurrenceChain` (`-recurrence-chain-limit`).
const MAX_RECURRENCE_CHAIN: usize = 3;

/// `body` with the recurrence of each loop-header phi commuted so the
/// phi's value is the tied operand (`PeepholeOptimizer::optimizeRecurrence`,
/// [PeepholeOptimizer.cpp#L1719](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PeepholeOptimizer.cpp#L1719),
/// `findTargetRecurrence` #L1648).
///
/// `s := phi(s0, s2); s1 := add t, s; s2 := add s1, u` ties `t` to `s1` and
/// `s1` to `s2`, so the copy phi elimination makes into `s` cannot coalesce
/// with `s2` (`s` and `t` overlap). Commuting the first add to `add s, t`
/// removes the copy (x_ll_arith: the sum's `adc` kept the shifted half as
/// its tied operand, +3 instructions in the loop).
///
/// A chain follows the single reader of each value while its instruction
/// has a tied def, up to three long, and counts only if it ends in one of the
/// phi's incoming values.
///
/// Not translated: nothing (the X86 commutes are `add`, `adc`, `and`, `or`,
/// `xor`, `imul`).
pub fn recurrences_commuted(body: &LirBody) -> LirBody {
    if body.blocks.iter().all(|block| block.phis.is_empty()) {
        return body.clone();
    }
    let forest = crate::analysis::loops::forest_of(&body.blocks, Some(body.entry));
    let mut reads: HashMap<u32, usize> = HashMap::default();
    let mut users: HashMap<u32, (usize, usize)> = HashMap::default();
    for (at, block) in body.blocks.iter().enumerate() {
        for phi in block.phis.iter() {
            for &(_from, value) in &phi.incoming {
                *reads.entry(value).or_default() += 1;
            }
        }
        for (place, one) in block.insns.iter().enumerate() {
            for value in one.uses.iter().copied().chain(one.requires.iter().map(|(held, _)| held.value)) {
                *reads.entry(value).or_default() += 1;
                users.insert(value, (at, place));
            }
        }
    }
    let mut swaps: HashSet<(usize, usize)> = HashSet::default();
    for block in body.blocks.iter() {
        if !forest.innermost(block.at).is_some_and(|one| forest.loops[one].header == block.at) {
            continue;
        }
        for phi in block.phis.iter() {
            let targets: HashSet<u32> = phi.incoming.iter().map(|&(_from, value)| value).collect();
            let mut chain = Vec::new();
            if target_recurrence(body, phi.result, &targets, &reads, &users, &mut chain) {
                swaps.extend(chain.into_iter().flatten());
            }
        }
    }
    if swaps.is_empty() {
        return body.clone();
    }
    let mut out = body.clone();
    for (at, place) in swaps {
        let one = &out.blocks[at].insns[place];
        let Some(what) = one.what.as_ref() else { continue };
        let mut made = (**one).clone();
        made.what = Some(Semantics { sources: vec![what.sources[1].clone(), what.sources[0].clone()], ..what.clone() });
        let mut insns: Vec<Arc<Insn>> = out.blocks[at].insns.iter().cloned().collect();
        insns[place] = Arc::new(made);
        out.blocks[at].insns = insns.into();
    }
    out
}

/// `findTargetRecurrence`: `chain` collects the instructions of the cycle, with
/// the place of each that reads the value in the second place (to commute).
fn target_recurrence(
    body: &LirBody,
    value: u32,
    targets: &HashSet<u32>,
    reads: &HashMap<u32, usize>,
    users: &HashMap<u32, (usize, usize)>,
    chain: &mut Vec<Option<(usize, usize)>>,
) -> bool {
    if targets.contains(&value) {
        return true;
    }
    if reads.get(&value).copied() != Some(1) || chain.len() >= MAX_RECURRENCE_CHAIN {
        return false;
    }
    let Some(&(at, place)) = users.get(&value) else { return false };
    let one = &body.blocks[at].insns[place];
    let Some(what) = one.what.as_ref() else { return false };
    let name = what.name.as_deref().unwrap_or_default();
    let tied = matches!(
        name,
        "add"
            | "sub"
            | "adc"
            | "sbb"
            | "and"
            | "or"
            | "xor"
            | "imul"
            | "neg"
            | "not"
            | "inc"
            | "dec"
            | "shl"
            | "shr"
            | "sar"
    );
    let ([Loc::Held(into)], Some(Loc::Held(first))) = (&what.dests[..], what.sources.first()) else { return false };
    if !tied || what.indirect || one.group.is_some() || !one.requires.is_empty() || !one.delivers.is_empty() {
        return false;
    }
    if first.value == value {
        chain.push(None);
        return into.width == first.width && target_recurrence(body, into.value, targets, reads, users, chain);
    }
    let commutes = matches!(name, "add" | "adc" | "and" | "or" | "xor" | "imul");
    let [_, Loc::Held(second)] = &what.sources[..] else { return false };
    if !commutes || second.value != value || into.width != first.width || first.width != second.width {
        return false;
    }
    chain.push(Some((at, place)));
    target_recurrence(body, into.value, targets, reads, users, chain)
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
            insn(
                0,
                Operation::Binary,
                "sub",
                vec![narrow(3, 2)],
                vec![narrow(1, 2), narrow(2, 2)],
                vec![3],
                vec![1, 2],
            ),
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

    fn constant(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    fn mov_const(
        at: i64,
        dest: u32,
        value: i64,
    ) -> Arc<Insn> {
        insn(at, Operation::Move, "mov", vec![held(dest)], vec![constant(value)], vec![dest], vec![])
    }

    /// `x_funcptr` at m32: the tail-duplicated dispatch read `1` and `0` from
    /// two `mov`s and added them (`lea esi, [edx+ebx]`); the pin makes it
    /// `add v2, 1`, which leaves the `mov 1` for no one, so the block's
    /// first instruction is the `xor eax, eax` of the loop-exit edge's
    /// block and the two are hoisted.
    #[test]
    fn test_an_add_of_a_constant_register_takes_the_immediate_form() {
        let body = body_of(vec![
            mov_const(0, 1, 1),
            mov_const(1, 2, 0),
            insn(2, Operation::Binary, "add", vec![held(3)], vec![held(1), held(2)], vec![3], vec![1, 2]),
            insn(3, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        let done = immediates_folded(&body, false);
        let insns = &done.blocks[0].insns;
        let add = insns[2].what.as_ref().unwrap();
        assert_eq!(
            add.sources,
            [held(2), constant(1)],
            "the first constant operand is the immediate, swapped to the end"
        );
        assert_eq!(insns[2].uses, [2]);
        assert!(insns[0].defines.is_empty(), "the mov of 1 has no reader left");
        assert_eq!(insns[1].defines, [2], "the mov of 0 is still read");
    }

    /// `sub` takes the immediate only as its second operand.
    #[test]
    fn test_a_subtract_from_a_constant_register_keeps_it() {
        let body = body_of(vec![
            mov_const(0, 1, 5),
            insn(1, Operation::Binary, "sub", vec![held(3)], vec![held(1), held(2)], vec![3], vec![1, 2]),
            insn(2, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(immediates_folded(&body, false), body);
    }

    /// For size a constant two instructions read stays a register.
    #[test]
    fn test_for_size_a_constant_read_twice_stays_a_register() {
        let body = body_of(vec![
            mov_const(0, 1, 7),
            insn(1, Operation::Binary, "add", vec![held(3)], vec![held(2), held(1)], vec![3], vec![2, 1]),
            insn(2, Operation::Binary, "and", vec![held(4)], vec![held(2), held(1)], vec![4], vec![2, 1]),
            insn(3, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(immediates_folded(&body, true), body);
        assert_ne!(immediates_folded(&body, false), body);
    }

    /// The pin's `COPY` fold: the copy of a constant register is a `mov` of the
    /// constant where the copy is, and the first `mov` goes when no one else
    /// reads it (x_funcptr: the `mov 1` that started the preheader moves to the
    /// end of it, behind the `add` that read it, and the `xor` is first).
    #[test]
    fn test_a_copy_of_a_constant_register_is_the_constant() {
        let body = body_of(vec![
            mov_const(0, 1, 1),
            insn(1, Operation::Move, "mov", vec![held(2)], vec![held(1)], vec![2], vec![1]),
            insn(2, Operation::Branch, "jmp", vec![], vec![], vec![], vec![]),
        ]);
        let done = immediates_folded(&body, false);
        let insns = &done.blocks[0].insns;
        assert_eq!(insns[1].what.as_ref().unwrap().sources, [constant(1)]);
        assert!(insns[0].defines.is_empty(), "the first mov has no reader left");
    }

    /// A zero is `xor`, which clobbers the flags: not ahead of a branch that
    /// reads them.
    #[test]
    fn test_a_copy_of_zero_before_a_flag_reader_stays() {
        let body = body_of(vec![
            mov_const(0, 1, 0),
            insn(1, Operation::Compare, "cmp", vec![], vec![held(4), held(5)], vec![], vec![4, 5]),
            insn(2, Operation::Move, "mov", vec![held(2)], vec![held(1)], vec![2], vec![1]),
            insn(3, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(immediates_folded(&body, false), body);
    }

    /// x_funcptr: the zero copies at the end of the preheader, ahead of the
    /// jump table, stay copies (the flags are unknown there, as in the
    /// pin), so the zero keeps its place at the head of the block.
    #[test]
    fn test_a_copy_of_zero_at_the_end_of_a_block_stays() {
        let body = body_of(vec![
            mov_const(0, 1, 0),
            insn(1, Operation::Move, "mov", vec![held(2)], vec![held(1)], vec![2], vec![1]),
            insn(2, Operation::Branch, "jmp", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(immediates_folded(&body, false), body);
    }

    /// With an `and` after it that writes the flags, the zero copy is folded.
    #[test]
    fn test_a_copy_of_zero_ahead_of_a_flag_write_is_folded() {
        let body = body_of(vec![
            mov_const(0, 1, 0),
            insn(1, Operation::Move, "mov", vec![held(2)], vec![held(1)], vec![2], vec![1]),
            insn(2, Operation::Binary, "and", vec![held(4)], vec![held(3), constant(3)], vec![4], vec![3]),
            insn(3, Operation::Branch, "jmp", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(immediates_folded(&body, false).blocks[0].insns[1].what.as_ref().unwrap().sources, [constant(0)]);
    }

    fn load_of(value: u32) -> Arc<Insn> {
        let cell = llrm_lir::Mem { index: Some(Held { value: 7, width: 4 }), scale: 4, ..llrm_lir::Mem::new(None, 4) };
        insn(0, Operation::Move, "mov", vec![held(value)], vec![Loc::Mem(cell)], vec![value], vec![7])
    }

    /// `v8 := mov [m]; cmp v8, v3; jl` kept the load in a register of its own
    /// (coalesce_hot_copies: a binary search ran 11 instructions a trip, the
    /// pin's 9); the pin's PeepholeOptimizer makes it `cmp [m], v3`.
    #[test]
    fn test_a_load_read_once_by_a_compare_is_its_operand() {
        let body = body_of(vec![
            load_of(8),
            insn(1, Operation::Compare, "cmp", vec![], vec![held(8), held(3)], vec![], vec![8, 3]),
            insn(2, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        let done = loads_folded(&body);
        assert_eq!(names(&done, true), ["", "cmp", "jl"]);
        let (load, cmp) = (&done.blocks[0].insns[0], &done.blocks[0].insns[1]);
        assert!(load.defines.is_empty() && load.uses.is_empty());
        assert!(matches!(cmp.what.as_ref().unwrap().sources[..], [Loc::Mem(_), Loc::Held(_)]));
        assert_eq!(cmp.uses, [7, 3]);
    }

    /// A load read twice, or with a store between it and its reader, stays.
    #[test]
    fn test_a_load_read_twice_or_past_a_store_stays() {
        let twice = body_of(vec![
            load_of(8),
            insn(1, Operation::Compare, "cmp", vec![], vec![held(8), held(3)], vec![], vec![8, 3]),
            insn(2, Operation::Compare, "cmp", vec![], vec![held(8), held(4)], vec![], vec![8, 4]),
        ]);
        assert!(loads_folded(&twice).blocks[0].insns[0].uses == [7]);
        let cell = llrm_lir::Mem::new(None, 4);
        let store = insn(1, Operation::Move, "mov", vec![Loc::Mem(cell)], vec![held(5)], vec![], vec![5]);
        let past = body_of(vec![
            load_of(8),
            store,
            insn(2, Operation::Compare, "cmp", vec![], vec![held(8), held(3)], vec![], vec![8, 3]),
        ]);
        assert!(matches!(loads_folded(&past).blocks[0].insns[2].what.as_ref().unwrap().sources[0], Loc::Held(_)));
    }

    /// regparm3_matrix_22 and _4: a word load read by a byte `add`/`xor` (the
    /// low byte of the value) folded to `add al, word ptr [bp+0xc]`, a width
    /// the encoder refuses (compile failure). The memory operand is the
    /// instruction's width, so only a load of that width folds.
    #[test]
    fn test_a_word_load_read_as_a_byte_is_not_folded() {
        let cell = llrm_lir::Mem::new(None, 2);
        let load = insn(
            0,
            Operation::Move,
            "mov",
            vec![Loc::Held(Held { value: 8, width: 2 })],
            vec![Loc::Mem(cell)],
            vec![8],
            vec![],
        );
        let byte = |value| Loc::Held(Held { value, width: 1 });
        let body = body_of(vec![
            load,
            insn(1, Operation::Binary, "add", vec![byte(4)], vec![byte(3), byte(8)], vec![4], vec![3, 8]),
        ]);
        let done = loads_folded(&body);
        assert!(matches!(done.blocks[0].insns[1].what.as_ref().unwrap().sources[..], [Loc::Held(_), Loc::Held(_)]));
    }

    /// A loop `s := phi(s0, s2); s1 := add t, s; s2 := add s1, u`.
    fn recurrence(
        name: &str,
        length: usize,
    ) -> LirBody {
        use crate::model::lir::Phi;
        let mut insns =
            vec![insn(0, Operation::Binary, "add", vec![held(20)], vec![held(5), held(5)], vec![20], vec![5, 5])];
        // v20 is the shifted half; each step adds one more operand to the sum
        let mut value = 10;
        for step in 0..length {
            let next = 30 + step as u32;
            let (first, second) = if step == 0 { (20, value) } else { (value, 21) };
            insns.push(insn(
                1 + step as i64,
                Operation::Binary,
                name,
                vec![held(next)],
                vec![held(first), held(second)],
                vec![next],
                vec![first, second],
            ));
            value = next;
        }
        let header = LirBlock {
            succ: vec![1, 2].into(),
            phis: vec![Phi { result: 10, incoming: vec![(0, 1), (1, value)] }].into(),
            ..LirBlock::new(1, insns)
        };
        let entry = LirBlock { succ: vec![1].into(), ..LirBlock::new(0, vec![]) };
        let mut body = LirBody::new(
            "f",
            0,
            vec![entry.into(), header.into(), LirBlock::new(2, vec![]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body
    }

    fn first_sources(
        body: &LirBody,
        place: usize,
    ) -> Vec<u32> {
        let what = body.blocks[1].insns[place].what.as_ref().unwrap();
        what.sources.iter().filter_map(|one| if let Loc::Held(held) = one { Some(held.value) } else { None }).collect()
    }

    /// x_ll_arith: the sum's `adc` took the shifted half as its tied operand
    /// and the loop-carried sum a copy (+3 instructions a trip, 43 against
    /// the pin's 40). The pin commutes the add that reads the phi.
    #[test]
    fn test_a_loops_sum_is_the_tied_operand_of_the_add_that_reads_it() {
        let body = recurrences_commuted(&recurrence("adc", 1));
        assert_eq!(first_sources(&body, 1), [10, 20]);
    }

    #[test]
    fn test_a_recurrence_longer_than_the_limit_is_left() {
        let body = recurrences_commuted(&recurrence("add", MAX_RECURRENCE_CHAIN + 1));
        assert_eq!(first_sources(&body, 1), [20, 10]);
    }

    #[test]
    fn test_a_subtract_is_not_commuted() {
        let body = recurrences_commuted(&recurrence("sub", 1));
        assert_eq!(first_sources(&body, 1), [20, 10]);
    }
}
