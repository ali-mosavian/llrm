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
//! DEVIATE LU1: the scan stays in the block; the pin follows a single-
//! predecessor successor into which the base is live (`VisitSucc`). Exit: port
//! it with the block liveness the pass has not got.
//! DEVIATE LU2: no red zone (`getRedZoneSize` is taken as 0), so an `sp`
//! pre-index never moves over an access.

use crate::mc::inst::*;
use crate::pairs::{is_memory, leaves_the_block};

/// `UpdateLimit` (`-aarch64-update-scan-limit`).
const UPDATE_LIMIT: usize = 100;

/// A load or store of one or two registers at `[base, #imm]`, as far as the
/// update merge goes.
struct Access {
    base: Gpr,
    imm: i64,
    /// The bytes of one register; `pair` doubles what the access moves.
    bytes: i64,
    pair: bool,
    /// The registers it moves.
    registers: Vec<crate::mc::visit::Reg>,
    /// The `ldur` family: the offset is not scaled.
    unscaled: bool,
}

fn reg_of(mem: &Memory) -> crate::mc::visit::Reg {
    match mem {
        Memory::Int { reg, .. } => crate::mc::visit::Reg::Gpr(*reg),
        Memory::Fp { reg, .. } => crate::mc::visit::Reg::Fpr(*reg),
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

/// Whether `inst` reads or writes `register`.
fn touches(
    inst: &Inst,
    register: crate::mc::visit::Reg,
) -> bool {
    let (defs, uses) = inst.defs_and_uses();
    defs.contains(&register) || uses.contains(&register)
}

/// `findMatchingUpdateInsnForward`: the index of the update after `at`.
fn update_after(
    insts: &[Option<Inst>],
    at: usize,
    access: &Access,
    offset: i64,
) -> Option<(usize, i64)> {
    // "If the base register overlaps a source/destination register, we can't
    // merge the update."
    let base = crate::mc::visit::Reg::Gpr(access.base);
    if access.registers.contains(&base) {
        return None;
    }
    // The immediate of the access is the amount the update must be, or the
    // access is at zero.
    if access.imm != offset {
        return None;
    }
    let stack = access.base == Gpr::R31;
    let mut count = 0;
    for (index, inst) in insts.iter().enumerate().skip(at + 1) {
        let Some(inst) = inst else { continue };
        if count >= UPDATE_LIMIT || leaves_the_block(inst) {
            return None;
        }
        count += 1;
        if let Some(amount) = update_of(inst, access, offset) {
            return Some((index, amount));
        }
        // "If the base register is used or modified, we have no match"; of the
        // stack pointer, any access in between.
        if touches(inst, base) || (stack && is_memory(inst)) {
            return None;
        }
    }
    None
}

/// `findMatchingUpdateInsnBackward`: the index of the update before `at`.
fn update_before(
    insts: &[Option<Inst>],
    at: usize,
    access: &Access,
) -> Option<(usize, i64)> {
    let base = crate::mc::visit::Reg::Gpr(access.base);
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

/// `tryToMergeLdStUpdate`.
fn merge_in(
    insts: Vec<Inst>,
    locs: Vec<Option<(u32, u32)>>,
) -> (Vec<Inst>, Vec<Option<(u32, u32)>>) {
    let mut insts: Vec<Option<Inst>> = insts.into_iter().map(Some).collect();
    for at in 0..insts.len() {
        let Some(access) = insts[at].as_ref().and_then(access_of) else { continue };
        // A post-index, then a pre-index from above, then one from below.
        let forward = update_after(&insts, at, &access, 0);
        let merge = if let Some((update, amount)) = forward {
            Some((update, amount, false))
        } else if access.unscaled {
            None
        } else if let Some((update, amount)) = update_before(&insts, at, &access) {
            Some((update, amount, true))
        } else {
            update_after(&insts, at, &access, access.imm).map(|(update, amount)| (update, amount, true))
        };
        let Some((update, amount, pre)) = merge else { continue };
        insts[at] = Some(written_back(insts[at].as_ref().expect("the access"), pre, amount));
        insts[update] = None;
    }
    insts.into_iter().zip(locs).filter_map(|(inst, loc)| Some((inst?, loc))).unzip()
}

/// `blocks` with the base updates of their accesses folded in.
pub fn fold(blocks: Vec<crate::frame::Lowered>) -> Vec<crate::frame::Lowered> {
    blocks
        .into_iter()
        .map(|block| {
            let (insts, locs) = merge_in(block.insts, block.locs);
            crate::frame::Lowered { label: block.label, insts, locs }
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
        merge_in(insts, places).0
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
}
