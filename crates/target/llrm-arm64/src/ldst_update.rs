//! A load or store and an `add`/`sub` of its base become one writeback access
//! (`AArch64LoadStoreOpt::tryToMergeLdStUpdate` with
//! `findMatchingUpdateInsnForward`, `findMatchingUpdateInsnBackward` and
//! `mergeUpdateInsn`):
//!
//! ```text
//! ldr x0, [x20]; add x20, x20, #32     ->  ldr x0, [x20], #32
//! add x0, x0, #8; ldr x1, [x0]         ->  ldr x1, [x0, #8]!
//! ldr x1, [x0, #64]; add x0, x0, #64   ->  ldr x1, [x0, #64]!
//! ```
//!
//! The forward scan follows a single-predecessor successor into which the base
//! is live and no other successor is (`VisitSucc`); the liveness is the base
//! register's alone, computed on demand.
//!
//! DEVIATE LU2: no red zone (`getRedZoneSize` is taken as 0), so an `sp`
//! pre-index never moves over an access.
//! DEVIATE LU3: a call ends the forward scan; the pin goes on over one when
//! the base survives it (callee-saved). Exit: none, a missed merge.
//! DEVIATE LX1: `tryToMergeIndexLdSt` is refused when the load's base is its
//! index register (`ldr w0, [x8, x8]`); the pin would add to a register whose
//! constant it just erased. Unreachable: `c + c` of a constant is folded
//! before selection, so no address is two equal constant registers. Exit: none.
//! DEVIATE LX2: the `movz` of the pair must shift by 0; the pin reads its
//! immediate without the shift. Unreachable: the immediate expansion puts the
//! lowest nonzero chunk in the `movz`, so a pair of two chunks has it at 0.
//! Exit: none.

use llrm_object::assembler::Cfi;
use llrm_support::hash::HashMap;

use crate::frame::Lowered;
use crate::layout::label_of;
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::pairs::{is_memory, leaves_the_block};

/// `UpdateLimit` (`-aarch64-update-scan-limit`).
const UPDATE_LIMIT: usize = 100;

/// `LdStConstLimit` (`-aarch64-load-store-const-scan-limit`).
const CONST_LIMIT: usize = 10;

/// A load or store of one or two registers at `[base, #imm]`, as far as the
/// update merge goes.
struct Access {
    base: Gpr,
    imm: i64,
    /// The bytes of one register; `pair` doubles what the access moves.
    bytes: i64,
    pair: bool,
    /// The registers it moves.
    registers: Vec<Reg>,
    /// The `ldur` family: the offset is not scaled.
    unscaled: bool,
}

fn reg_of(mem: &Memory) -> Reg {
    match mem {
        Memory::Int { reg, .. } => Reg::Gpr(*reg),
        Memory::Fp { reg, .. } => Reg::Fpr(*reg),
    }
}

fn size_of(mem: &Memory) -> i64 {
    match mem {
        Memory::Int { bytes, .. } | Memory::Fp { bytes, .. } => i64::from(*bytes),
    }
}

/// `isMergeableLdStUpdate`: the accesses with a pre- and post-indexed twin.
/// The sign-extending loads are not among them.
fn access_of(inst: &Inst) -> Option<Access> {
    match inst {
        Inst::Load { mem, addr: Addr::Offset { base, imm }, mmo }
        | Inst::Store { mem, addr: Addr::Offset { base, imm }, mmo } => {
            // "Do not form post-inc addressing mode for volatile accesses",
            // but for the stack.
            if mmo.volatile && *base != Gpr::R31 {
                return None;
            }
            if matches!(mem, Memory::Int { sign: true, .. }) {
                return None;
            }
            let bytes = size_of(mem);
            let scaled = *imm >= 0 && imm % bytes == 0 && imm / bytes < 4096;
            // `LDUR`/`STUR` exist for the S, D, Q, W and X registers only.
            if !scaled && !matches!(bytes, 4 | 8 | 16) {
                return None;
            }
            Some(Access { base: *base, imm: *imm, bytes, pair: false, registers: vec![reg_of(mem)], unscaled: !scaled })
        }
        Inst::LoadPair { first, second, addr: Addr::Offset { base, imm } }
        | Inst::StorePair { first, second, addr: Addr::Offset { base, imm } } => Some(Access {
            base: *base,
            imm: *imm,
            bytes: size_of(first),
            pair: true,
            registers: vec![reg_of(first), reg_of(second)],
            unscaled: false,
        }),
        _ => None,
    }
}

/// `getPrePostIndexedMemOpInfo`: the scale and the range of the immediate.
fn immediate_range(access: &Access) -> (i64, std::ops::RangeInclusive<i64>) {
    if access.pair { (access.bytes, -64..=63) } else { (1, -256..=255) }
}

/// `isMatchingUpdateInsn`: `add`/`sub base, base, #imm` that the access can
/// take, as the amount it adds; `offset` (when not zero) is the one it must
/// be.
fn update_of(
    inst: &Inst,
    access: &Access,
    offset: i64,
) -> Option<i64> {
    let Inst::AddSubImm { op, width: Width::X, rd, rn, imm12, shift12: false } = inst else { return None };
    if op.set_flags || *rd != access.base || *rn != access.base {
        return None;
    }
    let amount = if op.subtract { -i64::from(*imm12) } else { i64::from(*imm12) };
    let (scale, range) = immediate_range(access);
    if amount % scale != 0 || !range.contains(&(amount / scale)) {
        return None;
    }
    (offset == 0 || offset == amount).then_some(amount)
}

/// Whether `inst` reads or writes `sp`: register 31 in a slot that means the
/// stack pointer, not the zero register (`orr x9, xzr, x0` does not).
fn touches_sp(inst: &Inst) -> bool {
    let sp = |register: &Gpr| *register == Gpr::R31;
    let based = |addr: &Addr| match addr {
        Addr::Offset { base, .. }
        | Addr::PageOffset { base, .. }
        | Addr::Pre { base, .. }
        | Addr::Post { base, .. }
        | Addr::Register { base, .. } => sp(base),
        Addr::Slot { .. } => false,
    };
    match inst {
        Inst::AddSubImm { op, rd, rn, .. } | Inst::AddSubExtended { op, rd, rn, .. } => {
            sp(rn) || (!op.set_flags && sp(rd))
        }
        Inst::AddPageOffset { rd, rn, .. } => sp(rd) || sp(rn),
        Inst::LogicImm { rd, .. } => sp(rd),
        Inst::Load { addr, .. }
        | Inst::Store { addr, .. }
        | Inst::LoadPair { addr, .. }
        | Inst::StorePair { addr, .. } => based(addr),
        Inst::Atomic { rn, .. } => sp(rn),
        Inst::VectorStoreLane { base, .. } => sp(base),
        Inst::Prefetch { base, .. } => sp(base),
        _ => false,
    }
}

/// Whether `inst` writes `sp`: register 31 as the destination of a
/// non-flag-setting `add`/`sub`/`and`/`orr`, or as the base of a writeback
/// access.
pub(crate) fn writes_sp(inst: &Inst) -> bool {
    let sp = |register: &Gpr| *register == Gpr::R31;
    match inst {
        Inst::AddSubImm { op, rd, .. } | Inst::AddSubExtended { op, rd, .. } => !op.set_flags && sp(rd),
        Inst::AddPageOffset { rd, .. } | Inst::LogicImm { rd, .. } => sp(rd),
        Inst::Load { addr: Addr::Pre { base, .. } | Addr::Post { base, .. }, .. }
        | Inst::Store { addr: Addr::Pre { base, .. } | Addr::Post { base, .. }, .. }
        | Inst::LoadPair { addr: Addr::Pre { base, .. } | Addr::Post { base, .. }, .. }
        | Inst::StorePair { addr: Addr::Pre { base, .. } | Addr::Post { base, .. }, .. } => sp(base),
        _ => false,
    }
}

/// Whether `inst` reads or writes `register`.
fn touches(
    inst: &Inst,
    register: Reg,
) -> bool {
    if register == Reg::Gpr(Gpr::R31) {
        return touches_sp(inst);
    }
    let (defs, uses) = inst.defs_and_uses();
    defs.contains(&register) || uses.contains(&register)
}

/// `findMatchingUpdateInsnBackward`: the index of the update before `at`.
fn update_before(
    insts: &[Option<Inst>],
    at: usize,
    access: &Access,
) -> Option<(usize, i64)> {
    let base = Reg::Gpr(access.base);
    if access.imm != 0 || access.registers.contains(&base) {
        return None;
    }
    let stack = access.base == Gpr::R31;
    let mut count = 0;
    let mut access_between = false;
    for index in (0..at).rev() {
        let Some(inst) = &insts[index] else { continue };
        count += 1;
        if let Some(amount) = update_of(inst, access, 0) {
            // LU2: the red zone is 0.
            return (!(stack && access_between && amount != 0)).then_some((index, amount));
        }
        // The merged access takes the place of the access, so what lies
        // between may use memory and the access's registers, but not the base.
        if count >= UPDATE_LIMIT || leaves_the_block(inst) || touches(inst, base) {
            return None;
        }
        access_between |= is_memory(inst);
    }
    None
}

/// The access with its address a writeback form of `amount`.
fn written_back(
    inst: &Inst,
    pre: bool,
    amount: i64,
) -> Inst {
    let with = |addr: &Addr| {
        let Addr::Offset { base, .. } = addr else { unreachable!("an access at base and offset") };
        if pre { Addr::Pre { base: *base, imm: amount } } else { Addr::Post { base: *base, imm: amount } }
    };
    match inst {
        Inst::Load { mem, addr, mmo } => Inst::Load { mem: *mem, addr: with(addr), mmo: mmo.clone() },
        Inst::Store { mem, addr, mmo } => Inst::Store { mem: *mem, addr: with(addr), mmo: mmo.clone() },
        Inst::LoadPair { first, second, addr } => Inst::LoadPair { first: *first, second: *second, addr: with(addr) },
        Inst::StorePair { first, second, addr } => Inst::StorePair { first: *first, second: *second, addr: with(addr) },
        _ => unreachable!("an access"),
    }
}

/// The blocks of a function, their instructions optionally deleted.
struct Function<'a> {
    blocks: Vec<Vec<Option<Inst>>>,
    /// The source place of each instruction, parallel to `blocks`.
    locs: Vec<Vec<Option<(u32, u32)>>>,
    labels: HashMap<&'a str, usize>,
    /// The registers the function leaves live: what it returns and passes to a
    /// tail call.
    exits: &'a [Reg],
    /// A register's live-in blocks, once asked for.
    live: HashMap<Gpr, Vec<bool>>,
}

/// Whether a call clobbers `register` (the caller-saved x0-x17 and the
/// platform register) and reads it (the arguments).
fn call_clobbers(register: Gpr) -> bool {
    register.0 <= 18
}

fn call_reads(register: Gpr) -> bool {
    register.0 <= 7
}

fn is_call(inst: &Inst) -> bool {
    matches!(
        inst,
        Inst::Branch { link: true, .. } | Inst::BranchReg { link: true, .. }
    )
}

impl Function<'_> {
    fn successors(
        &self,
        block: usize,
    ) -> Vec<usize> {
        let insts = &self.blocks[block];
        let mut out: Vec<usize> =
            insts.iter().flatten().filter_map(label_of).filter_map(|label| self.labels.get(label).copied()).collect();
        let ends = matches!(
            insts.iter().flatten().last(),
            Some(
                Inst::Branch { link: false, .. }
                    | Inst::Ret { .. }
                    | Inst::BranchReg { link: false, .. }
                    | Inst::Brk { .. }
            )
        );
        if !ends && block + 1 < self.blocks.len() {
            out.push(block + 1);
        }
        out
    }

    fn predecessors(
        &self,
        block: usize,
    ) -> usize {
        (0..self.blocks.len()).filter(|other| self.successors(*other).contains(&block)).count()
    }

    /// Whether `register` is read by a path from the start of each block
    /// before it is written.
    fn live_in(
        &mut self,
        register: Gpr,
    ) -> &[bool] {
        if !self.live.contains_key(&register) {
            let successors: Vec<Vec<usize>> = (0..self.blocks.len()).map(|block| self.successors(block)).collect();
            let leaves = self.exits.contains(&Reg::Gpr(register));
            let mut live = vec![false; self.blocks.len()];
            let mut changed = true;
            while changed {
                changed = false;
                for block in (0..self.blocks.len()).rev() {
                    // A block with no successor leaves the function.
                    let mut now = if successors[block].is_empty() {
                        leaves
                    } else {
                        successors[block].iter().any(|next| live[*next])
                    };
                    for inst in self.blocks[block].iter().flatten().rev() {
                        let (defs, uses) = inst.defs_and_uses();
                        let (def, used) = (defs.contains(&Reg::Gpr(register)), uses.contains(&Reg::Gpr(register)));
                        if is_call(inst) {
                            now = (now && !call_clobbers(register)) || call_reads(register) || used;
                        } else {
                            now = used || (now && !def);
                        }
                    }
                    if now && !live[block] {
                        live[block] = true;
                        changed = true;
                    }
                }
            }
            self.live.insert(register, live);
        }
        &self.live[&register]
    }

    /// `findMatchingUpdateInsnForward`: the block and index of the update after
    /// `at` in `block`, and its amount.
    fn update_after(
        &mut self,
        block: usize,
        at: usize,
        access: &Access,
        offset: i64,
    ) -> Option<(usize, usize, i64)> {
        // "If the base register overlaps a source/destination register, we
        // can't merge the update."
        let base = Reg::Gpr(access.base);
        if access.registers.contains(&base) || access.imm != offset {
            return None;
        }
        let stack = access.base == Gpr::R31;
        let (mut here, mut from, mut count) = (block, at + 1, 0);
        loop {
            for index in from..self.blocks[here].len() {
                let Some(inst) = &self.blocks[here][index] else { continue };
                if count >= UPDATE_LIMIT {
                    return None;
                }
                // Debug and frame marks are not counted ("transient").
                if !matches!(inst, Inst::Cfi(_) | Inst::Mark(_) | Inst::Nop) {
                    count += 1;
                }
                if let Some(amount) = update_of(inst, access, offset) {
                    return Some((here, index, amount));
                }
                // "If the base register is used or modified, we have no match";
                // of the stack pointer, any access in between. LU3.
                if is_call(inst) || touches(inst, base) || (stack && is_memory(inst)) {
                    return None;
                }
            }
            if count >= UPDATE_LIMIT || stack {
                return None;
            }
            // "Try to go downward to successors along a CF path w/o side enters
            // such that BaseReg is alive along it but not at its exits."
            let mut next = None;
            let mut live_successors = 0;
            for successor in self.successors(here) {
                if self.live_in(access.base)[successor] {
                    live_successors += 1;
                    if live_successors > 1 {
                        return None;
                    }
                    if self.predecessors(successor) == 1 {
                        next = Some(successor);
                    }
                }
            }
            (here, from) = (next?, 0);
        }
    }
}

impl Function<'_> {
    /// `maybeMoveCFI`: the stack pointer now moves at the merged access, so
    /// the `DW_CFA_def_cfa*` that followed the deleted update at `update`
    /// follows the access at `at` instead; the frame it states is the one the
    /// instructions between do not yet have.
    fn move_cfi(
        &mut self,
        block: usize,
        update: usize,
        at: usize,
    ) {
        let insts = &self.blocks[block];
        let Some(cfi) = (update + 1..insts.len()).find(|&index| insts[index].is_some()) else { return };
        if cfi == at || !matches!(
            &insts[cfi],
            Some(Inst::Cfi(Cfi::DefCfaOffset { .. } | Cfi::DefCfa { .. }))
        ) {
            return;
        }
        // Past the access, whichever side the update was on.
        let (blocks, locs) = (&mut self.blocks[block], &mut self.locs[block]);
        if cfi < at {
            blocks[cfi..=at].rotate_left(1);
            locs[cfi..=at].rotate_left(1);
        } else {
            blocks[at + 1..=cfi].rotate_right(1);
            locs[at + 1..=cfi].rotate_right(1);
        }
    }
}

impl Function<'_> {
    /// Whether `register` is dead after `block[at]`: no later instruction of
    /// the block reads it before writing it, and no block control may reach
    /// reads it (the kill flag `findMatchingConstOffsetBackward` asks for).
    fn dead_after(
        &mut self,
        block: usize,
        at: usize,
        register: Gpr,
    ) -> bool {
        let key = Reg::Gpr(register);
        for index in at + 1..self.blocks[block].len() {
            let Some(inst) = &self.blocks[block][index] else { continue };
            let (defs, uses) = inst.defs_and_uses();
            if is_call(inst) {
                return !uses.contains(&key) && !call_reads(register) && call_clobbers(register);
            }
            if uses.contains(&key) {
                return false;
            }
            if leaves_the_block(inst) {
                return !self.live_out(block, register);
            }
            if defs.contains(&key) {
                return true;
            }
        }
        !self.live_out(block, register)
    }

    /// Whether `register` is read from the start of a block that control goes
    /// to from `block`, or is left live by the function.
    fn live_out(
        &mut self,
        block: usize,
        register: Gpr,
    ) -> bool {
        let successors = self.successors(block);
        if successors.is_empty() {
            return self.exits.contains(&Reg::Gpr(register));
        }
        let live = self.live_in(register);
        successors.iter().any(|next| live[*next])
    }
}

/// `isMatchingMovConstInsn` with `findMatchingConstOffsetBackward`: the
/// `movz`/`movk` pair before `at` that builds `index` (at most `CONST_LIMIT`
/// instructions back, `index` not otherwise used), as the indices of the two
/// and the constant. Only a pair under 2^24 is a pair of immediates an `add`
/// takes.
fn const_offset_before(
    insts: &[Option<Inst>],
    at: usize,
    index: Gpr,
) -> Option<(usize, usize, u32)> {
    let mut count = 0;
    for here in (0..at).rev() {
        let Some(inst) = &insts[here] else { continue };
        if !matches!(inst, Inst::Cfi(_) | Inst::Mark(_) | Inst::Nop) {
            count += 1;
        }
        if let Inst::MoveWide { op: Wide::Keep, width: Width::W, rd, imm16: high, lsl } = inst {
            if *rd == index {
                let zero = (0..here).rev().find(|&before| insts[before].is_some())?;
                if let Some(Inst::MoveWide { op: Wide::Zero, width: Width::W, rd: zrd, imm16: low, lsl: 0 }) =
                    &insts[zero]
                {
                    if zrd == rd {
                        let offset = (u32::from(*high) << lsl) + u32::from(*low);
                        return (offset >> 24 == 0).then_some((zero, here, offset));
                    }
                }
            }
        }
        if touches(inst, Reg::Gpr(index)) || count >= CONST_LIMIT {
            return None;
        }
    }
    None
}

/// `tryToMergeIndexLdSt`: a load at `[base, index]` whose `index` is a
/// `movz`/`movk` constant and dies there becomes an `add` of the high part to
/// the base and a load at the low 12 bits (scaled):
///
/// ```text
/// mov x8, #17767; movk x8, #291, lsl #16; ldr x0, [x1, x8]
///     -> add x8, x1, #4, lsl #12; ldr x0, [x8, #4464]
/// ```
///
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64LoadStoreOptimizer.cpp#L3002
fn merge_index_in(
    function: &mut Function<'_>,
    block: usize,
) {
    let mut at = 0;
    while at < function.blocks[block].len() {
        let step = merge_index_at(function, block, at);
        at += if step { 2 } else { 1 };
    }
}

fn merge_index_at(
    function: &mut Function<'_>,
    block: usize,
    at: usize,
) -> bool {
    // `isMergeableIndexLdSt`: the loads at an X register, unshifted.
    let Some(Inst::Load {
        mem,
        addr: Addr::Register { base, index, extend: Extend::Uxtx | Extend::Sxtx, shifted: false },
        mmo,
    }) = function.blocks[block][at].clone()
    else {
        return false;
    };
    let scale = size_of(&mem) as u32;
    let Some((zero, keep, offset)) = const_offset_before(&function.blocks[block], at, index) else { return false };
    // The pin never meets `[x8, x8]`: the index would be its own base.
    if base == index || offset & (scale - 1) != 0 || !function.dead_after(block, at, index) {
        return false;
    }
    let low = offset & (4096 * scale - 1);
    let high = offset - low;
    let add = Inst::AddSubImm {
        op: AddSub { subtract: false, set_flags: false },
        width: Width::X,
        rd: index,
        rn: base,
        imm12: (high >> 12) as u16,
        shift12: true,
    };
    let loc = function.locs[block][at];
    function.blocks[block][at] = Some(Inst::Load { mem, addr: Addr::Offset { base: index, imm: i64::from(low) }, mmo });
    function.blocks[block].insert(at, Some(add));
    function.locs[block].insert(at, loc);
    function.blocks[block][keep] = None;
    function.blocks[block][zero] = None;
    function.live.clear();
    true
}

/// `tryToMergeLdStUpdate`.
fn merge_in(
    function: &mut Function<'_>,
    block: usize,
) {
    for at in 0..function.blocks[block].len() {
        let Some(access) = function.blocks[block][at].as_ref().and_then(access_of) else { continue };
        // A post-index, then a pre-index from above, then one from below.
        let forward = function.update_after(block, at, &access, 0);
        let merge = if let Some((update_block, update, amount)) = forward {
            Some((update_block, update, amount, false))
        } else if access.unscaled {
            None
        } else if let Some((update, amount)) = update_before(&function.blocks[block], at, &access) {
            Some((block, update, amount, true))
        } else {
            function
                .update_after(block, at, &access, access.imm)
                .map(|(update_block, update, amount)| (update_block, update, amount, true))
        };
        let Some((update_block, update, amount, pre)) = merge else { continue };
        let written = written_back(function.blocks[block][at].as_ref().expect("the access"), pre, amount);
        function.blocks[block][at] = Some(written);
        function.blocks[update_block][update] = None;
        if access.base == Gpr::R31 && update_block == block {
            function.move_cfi(block, update, at);
        }
        // The deleted update was a base change the liveness may have seen.
        function.live.clear();
    }
}

/// `blocks` with the base updates of their accesses folded in. `exits` are the
/// registers the function returns and passes to a tail call.
pub fn fold(
    blocks: Vec<Lowered>,
    exits: &[Reg],
) -> Vec<Lowered> {
    let labels: Vec<String> = blocks.iter().map(|block| block.label.clone()).collect();
    let mut function = Function {
        blocks: blocks.iter().map(|block| block.insts.iter().cloned().map(Some).collect()).collect(),
        locs: blocks.iter().map(|block| block.locs.clone()).collect(),
        labels: labels.iter().enumerate().map(|(at, label)| (label.as_str(), at)).collect(),
        exits,
        live: HashMap::default(),
    };
    for block in 0..function.blocks.len() {
        merge_in(&mut function, block);
    }
    for block in 0..function.blocks.len() {
        merge_index_in(&mut function, block);
    }
    let kept = function.blocks.into_iter().zip(function.locs);
    blocks
        .into_iter()
        .zip(kept)
        .map(|(block, (insts, places))| {
            let (insts, locs) = insts.into_iter().zip(places).filter_map(|(inst, loc)| Some((inst?, loc))).unzip();
            Lowered { label: block.label, insts, locs }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(
        bytes: u8,
        reg: u32,
    ) -> Memory {
        Memory::Int { width: if bytes == 8 { Width::X } else { Width::W }, bytes, sign: false, reg: Gpr(reg) }
    }

    fn at(
        mem: Memory,
        load: bool,
        base: u32,
        imm: i64,
    ) -> Inst {
        let addr = Addr::Offset { base: Gpr(base), imm };
        if load {
            Inst::Load { mem, addr, mmo: MemOperand::default() }
        } else {
            Inst::Store { mem, addr, mmo: MemOperand::default() }
        }
    }

    fn step(
        base: u32,
        amount: i64,
    ) -> Inst {
        Inst::AddSubImm {
            op: AddSub { subtract: amount < 0, set_flags: false },
            width: Width::X,
            rd: Gpr(base),
            rn: Gpr(base),
            imm12: amount.unsigned_abs() as u16,
            shift12: false,
        }
    }

    fn merged(insts: Vec<Inst>) -> Vec<Inst> {
        let places = vec![None; insts.len()];
        let block = Lowered { label: "b".into(), insts, locs: places };
        fold(vec![block], &[]).remove(0).insts
    }

    /// The folded stack bump of QB `floats` sat after two ALU instructions
    /// while the frame was stated before them (`cfi_check`: the rows said
    /// `sp+16` over pc 0..12, where `sp` had not moved). As `maybeMoveCFI`,
    /// the frame follows the access that moves the stack pointer.
    #[test]
    fn a_frame_stated_after_an_update_follows_the_access_it_merges_into() {
        let out = merged(vec![
            step(31, -16),
            Inst::Cfi(Cfi::DefCfaOffset { offset: 16 }),
            Inst::Nop,
            at(int(8, 10), false, 31, 0),
        ]);
        let cfi = out.iter().position(|inst| matches!(inst, Inst::Cfi(_))).expect("the frame is stated");
        let store = out
            .iter()
            .position(|inst| matches!(
                inst,
                Inst::Store { addr: Addr::Pre { base: Gpr(31), imm: -16 }, .. }
            ))
            .expect("the bump is folded");
        assert!(cfi > store, "{out:?}");
    }

    /// The same for the epilogue: the frame is gone at the access that pops it.
    #[test]
    fn a_frame_dropped_after_an_update_follows_the_post_index_it_merges_into() {
        let out = merged(vec![
            at(int(8, 10), true, 31, 0),
            Inst::Nop,
            step(31, 16),
            Inst::Cfi(Cfi::DefCfaOffset { offset: 0 }),
        ]);
        let cfi = out.iter().position(|inst| matches!(inst, Inst::Cfi(_))).expect("the frame is stated");
        let load = out
            .iter()
            .position(|inst| matches!(
                inst,
                Inst::Load { addr: Addr::Post { base: Gpr(31), imm: 16 }, .. }
            ))
            .expect("the pop is folded");
        assert_eq!(cfi, load + 1, "{out:?}");
    }

    /// `ldr x0, [x20]; add x20, x20, #32` stayed two instructions.
    #[test]
    fn an_add_after_a_load_is_a_post_index() {
        let out = merged(vec![at(int(8, 0), true, 20, 0), step(20, 32)]);
        assert!(matches!(&out[..], [Inst::Load { addr: Addr::Post { base: Gpr(20), imm: 32 }, .. }]), "{out:?}");
    }

    /// The pointer step of a halfword copy loop was a separate `add`: the
    /// `strh w9, [x0]; add x0, x0, #2` of scroll.
    #[test]
    fn a_halfword_store_takes_the_step_of_its_pointer() {
        let out = merged(vec![at(int(2, 9), false, 0, 0), step(0, 2)]);
        assert!(matches!(&out[..], [Inst::Store { addr: Addr::Post { imm: 2, .. }, .. }]), "{out:?}");
    }

    #[test]
    fn an_add_before_an_access_at_zero_is_a_pre_index() {
        let out = merged(vec![step(0, 8), at(int(8, 1), true, 0, 0)]);
        assert!(matches!(&out[..], [Inst::Load { addr: Addr::Pre { imm: 8, .. }, .. }]), "{out:?}");
    }

    /// `sub sp, sp, #96; orr x9, xzr, x0; str x10, [sp]` stayed apart: register
    /// 31 of the `orr` (the zero register) read as `sp`, which cost 4 bytes on
    /// fpbench; the pin makes `str x10, [sp, #-96]!`.
    #[test]
    fn a_move_from_the_zero_register_does_not_stop_a_stack_pre_index() {
        let mov = Inst::LogicShifted {
            op: Logic::Orr,
            invert: false,
            width: Width::X,
            rd: Gpr(9),
            rn: Gpr(31),
            rm: Gpr(0),
            shift: Shift::Lsl,
            amount: 0,
        };
        let out = merged(vec![step(31, -96), mov, at(int(8, 10), false, 31, 0)]);
        assert!(matches!(&out[1], Inst::Store { addr: Addr::Pre { imm: -96, .. }, .. }), "{out:?}");
    }

    #[test]
    fn an_add_by_the_offset_after_an_access_is_a_pre_index() {
        let out = merged(vec![at(int(8, 1), true, 0, 64), step(0, 64)]);
        assert!(matches!(&out[..], [Inst::Load { addr: Addr::Pre { imm: 64, .. }, .. }]), "{out:?}");
    }

    #[test]
    fn a_use_of_the_base_between_stops_the_merge() {
        let out = merged(vec![at(int(8, 0), true, 20, 0), at(int(8, 1), true, 20, 8), step(20, 16)]);
        assert_eq!(out.len(), 3, "{out:?}");
    }

    /// `ldr x0, [x0]; add x0, x0, #8` would write the base twice.
    #[test]
    fn a_load_into_its_base_is_left_alone() {
        let out = merged(vec![at(int(8, 0), true, 0, 0), step(0, 8)]);
        assert_eq!(out.len(), 2, "{out:?}");
    }

    #[test]
    fn a_step_beyond_nine_bits_is_left_alone() {
        let out = merged(vec![at(int(8, 0), true, 20, 0), step(20, 256)]);
        assert_eq!(out.len(), 2, "{out:?}");
    }

    #[test]
    fn a_volatile_access_is_left_alone() {
        let volatile = Inst::Store {
            mem: int(8, 0),
            addr: Addr::Offset { base: Gpr(20), imm: 0 },
            mmo: MemOperand { volatile: true, ..MemOperand::default() },
        };
        let out = merged(vec![volatile, step(20, 8)]);
        assert_eq!(out.len(), 2, "{out:?}");
    }

    #[test]
    fn a_pair_takes_a_step_by_a_multiple_of_its_size() {
        let pair = Inst::LoadPair { first: int(8, 1), second: int(8, 2), addr: Addr::Offset { base: Gpr(0), imm: 0 } };
        let out = merged(vec![pair.clone(), step(0, 16)]);
        assert!(matches!(&out[..], [Inst::LoadPair { addr: Addr::Post { imm: 16, .. }, .. }]), "{out:?}");
        assert_eq!(merged(vec![pair, step(0, 12)]).len(), 2);
    }

    fn branch_to(label: &str) -> Inst {
        Inst::Branch { link: false, target: Target::Label(label.into()) }
    }

    fn cond_to(label: &str) -> Inst {
        Inst::BranchCond { cond: Cond::Eq, target: Target::Label(label.into()) }
    }

    fn block(
        label: &str,
        insts: Vec<Inst>,
    ) -> Lowered {
        Lowered { label: label.into(), locs: vec![None; insts.len()], insts }
    }

    fn leave() -> Inst {
        Inst::Ret { rn: Gpr::LR }
    }

    /// `ldr w12, [x9]; b.eq exit; ...; add x9, x9, #4` in the next block: the
    /// pin's scan follows a successor with one predecessor into which the base
    /// is live (queens' search loop kept its `add` and had no post-index).
    #[test]
    fn the_update_in_the_only_live_successor_is_merged() {
        let blocks = vec![
            block("head", vec![at(int(4, 12), true, 9, 0), cond_to("exit"), branch_to("body")]),
            block("body", vec![step(9, 4), branch_to("head")]),
            block("exit", vec![leave()]),
        ];
        let out = fold(blocks, &[]);
        assert!(matches!(&out[0].insts[0], Inst::Load { addr: Addr::Post { imm: 4, .. }, .. }), "{out:?}");
        assert_eq!(out[1].insts.len(), 1, "{out:?}");
    }

    /// The path out through `exit` reads the base: moving the step above the
    /// branch would change what it reads.
    #[test]
    fn an_update_is_not_moved_above_a_branch_to_a_block_that_reads_the_base() {
        let blocks = vec![
            block("head", vec![at(int(4, 12), true, 9, 0), cond_to("exit"), branch_to("body")]),
            block("body", vec![step(9, 4), branch_to("head")]),
            block("exit", vec![at(int(4, 1), true, 9, 0), leave()]),
        ];
        let out = fold(blocks, &[]);
        assert!(matches!(&out[0].insts[0], Inst::Load { addr: Addr::Offset { .. }, .. }), "{out:?}");
        assert_eq!(out[1].insts.len(), 2, "{out:?}");
    }

    /// A function that returns the base leaves it live at the `ret`.
    #[test]
    fn a_register_the_function_returns_is_live_at_its_exit() {
        let blocks = vec![
            block("head", vec![at(int(4, 12), true, 0, 0), cond_to("exit"), branch_to("body")]),
            block("body", vec![step(0, 4), branch_to("head")]),
            block("exit", vec![leave()]),
        ];
        let out = fold(blocks, &[Reg::Gpr(Gpr(0))]);
        assert!(matches!(&out[0].insts[0], Inst::Load { addr: Addr::Offset { .. }, .. }), "{out:?}");
    }

    fn wide(
        op: Wide,
        reg: u32,
        imm16: u16,
        lsl: u8,
    ) -> Inst {
        Inst::MoveWide { op, width: Width::W, rd: Gpr(reg), imm16, lsl }
    }

    fn load_at(
        bytes: u8,
        base: u32,
        index: u32,
    ) -> Inst {
        Inst::Load {
            mem: int(bytes, 0),
            addr: Addr::Register { base: Gpr(base), index: Gpr(index), extend: Extend::Uxtx, shifted: false },
            mmo: MemOperand::default(),
        }
    }

    /// `mov w8, #4464; movk w8, #1, lsl #16; ldr w0, [x1, x8]` (offset 70000)
    /// stayed three instructions; the pin's `aarch64-ldst-opt` makes it
    /// `add x8, x1, #16, lsl #12; ldr w0, [x8, #4464]`.
    #[test]
    fn a_dead_movz_movk_index_becomes_an_add_and_an_offset() {
        let out = merged(vec![
            wide(Wide::Zero, 8, 4464, 0),
            wide(Wide::Keep, 8, 1, 16),
            load_at(4, 1, 8),
            Inst::Ret { rn: Gpr(30) },
        ]);
        assert!(
            matches!(
                &out[..],
                [
                    Inst::AddSubImm { rd: Gpr(8), rn: Gpr(1), imm12: 16, shift12: true, .. },
                    Inst::Load { addr: Addr::Offset { base: Gpr(8), imm: 4464 }, .. },
                    Inst::Ret { .. },
                ]
            ),
            "{out:?}"
        );
    }

    /// The index is read again: the pin needs its kill flag, so nothing merges.
    #[test]
    fn a_live_index_keeps_the_register_form() {
        let out = merged(vec![
            wide(Wide::Zero, 8, 4464, 0),
            wide(Wide::Keep, 8, 1, 16),
            load_at(4, 1, 8),
            load_at(4, 2, 8),
            Inst::Ret { rn: Gpr(30) },
        ]);
        assert_eq!(out.len(), 5, "{out:?}");
        assert!(out.iter().any(|inst| matches!(inst, Inst::MoveWide { op: Wide::Keep, .. })), "{out:?}");
    }

    /// An offset the access size does not divide, one of 2^24 or more, and a
    /// pair further than `CONST_LIMIT` instructions back all stay.
    #[test]
    fn an_offset_the_pin_leaves_is_left() {
        let unaligned = merged(vec![wide(Wide::Zero, 8, 4465, 0), wide(Wide::Keep, 8, 1, 16), load_at(4, 1, 8)]);
        assert_eq!(unaligned.len(), 3, "{unaligned:?}");
        let wide_offset = merged(vec![wide(Wide::Zero, 8, 4464, 0), wide(Wide::Keep, 8, 256, 16), load_at(4, 1, 8)]);
        assert_eq!(wide_offset.len(), 3, "{wide_offset:?}");
        let mut far = vec![wide(Wide::Zero, 8, 4464, 0), wide(Wide::Keep, 8, 1, 16)];
        far.extend(std::iter::repeat_with(|| step(5, 1)).take(CONST_LIMIT));
        far.push(load_at(4, 1, 8));
        let len = far.len();
        assert_eq!(merged(far).len(), len);
    }

    /// A pair whose high chunk is zero (`movk w8, #0, lsl #16`) gives an `add
    /// x8, x1, #0, lsl #12`. The pin does the same (`MOVZWi 4464; MOVKWi 0,
    /// 16; LDRWroX` -> `ADDXri $x0, 0, 12; LDRWui 1116`, `-run-pass=
    /// aarch64-ldst-opt`), so it is kept. Unreachable: the immediate
    /// expansion writes no zero chunk, so a constant under 2^16 is one `movz`
    /// and no pair (`a_constant_in_one_chunk_is_no_pair`).
    #[test]
    fn a_pair_with_a_zero_high_chunk_adds_zero_as_the_pin_does() {
        let out = merged(vec![
            wide(Wide::Zero, 8, 4464, 0),
            wide(Wide::Keep, 8, 0, 16),
            load_at(4, 1, 8),
            Inst::Ret { rn: Gpr(30) },
        ]);
        assert!(
            matches!(
                &out[..],
                [
                    Inst::AddSubImm { rd: Gpr(8), rn: Gpr(1), imm12: 0, shift12: true, .. },
                    Inst::Load { addr: Addr::Offset { base: Gpr(8), imm: 4464 }, .. },
                    Inst::Ret { .. },
                ]
            ),
            "{out:?}"
        );
    }

    /// A `movz` alone is no pair (`isMatchingMovConstInsn` needs the `movk`),
    /// so a constant under 2^16 never reaches the merge, which would add 0.
    #[test]
    fn a_constant_in_one_chunk_is_no_pair() {
        let out = merged(vec![wide(Wide::Zero, 8, 4097, 0), load_at(1, 1, 8), Inst::Ret { rn: Gpr(30) }]);
        assert_eq!(out.len(), 3, "{out:?}");
        assert!(out.iter().all(|inst| !matches!(inst, Inst::AddSubImm { .. })), "{out:?}");
    }
}
