//! Recognises allocated instructions that keep a register an affine sum of
//! registers, and names the 32-bit or 16-bit address of such a sum.
//!
//! # How it works
//! A register is affine when each write to it is a copy, an add of a
//! constant, a scale or an add of another register: `step` classifies one
//! instruction as `Copy`, `Add`, `Scale` or `AddRegister` and prices it from
//! the CPU profile. Only 16- and 32-bit integer registers qualify, and an
//! instruction with side effects beyond its operands (`plain`) never does.
//!
//! A chain begins at a copy, or at a symbol-free `lea` (`lea_start`), and its
//! steps may be words or the dwords the target ran them as (`promote`). A
//! chain of such steps leaves a register equal to `sum(multiple * reg) +
//! disp`, kept as `Terms`. `form` turns terms into the address-size-
//! prefixed 32-bit address that names them (`[base+index*scale+disp]`, the
//! index never the stack pointer, the scale one the target's form takes).
//! `word_form` does the same for real-mode addressing: a lone BX/BP/SI/DI,
//! or one base and one index, with no scale and no prefix.
//!
//! ```text
//!  mov si,ax ; shl si,1 ; add si,4     terms [(ax,2)], disp 4
//!                                      form  [eax*2+4]  (67h)
//! ```
//!
//! # Example
//! A copy and a shift that make `cx*2`, before and after the peephole's address
//! chain. `; ...` marks lines left out; the `; inserted` notes are dropped.
//!
//! Before (`ParallelCopy`):
//!
//! ```lir
//! 0x0000  CX := mov [None v2@SI]
//! ; ...
//! 0x0007  BX := mov CX
//! 0x0007  BX := shl BX, 0x1
//! ```
//!
//! After (`Peephole`):
//!
//! ```lir
//! 0x0000  CX := mov [None v2@SI]
//! ; ...
//! 0x0007  BX := lea Address(addr=None, through=38, index=38, scale=1, offset=0, disp_width=0)
//! ```
//!
//! The chain is one `lea` in the listing: `lea bx, [ecx+ecx]`.
//!
//! From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of a BASIC function that sums an
//! INTEGER array (`Total%`).
//!
//! # Used by
//! `exactaddress.rs`, which folds the chain into a memory operand, and the
//! peephole's address chains (`peephole.rs`), which replace a chain by one
//! `lea`. Each does so only where the address is cheaper than the chain.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::backend::cpu::Profile;
use crate::model::ir::{AddressRef, Imm, Loc, Operation, Reg};
use crate::model::lir::Insn;

/// What one instruction makes of the register it writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// `mov z,y`.
    Copy(Reg),
    /// `add`/`sub` of a constant, `inc`, `dec`.
    Add(i64),
    /// `shl` by a constant, `add z,z`.
    Scale(i64),
    /// `add z,y`.
    AddRegister(Reg),
}

/// One register's multiple, as an affine chain computes it.
pub type Terms = Vec<(RegId, i64)>;

/// Whether `one` has no effect beyond its operands.
pub fn plain(one: &Insn) -> bool {
    one.what.is_some()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.spread.is_empty()
        && one.group.is_none()
        && !one.frame_adjust
}

/// The register `one` writes, how, and what `cpu` charges for it, where the
/// register stays an affine sum.
pub fn step(
    regs: Regs,
    one: &Insn,
    cpu: &Profile,
) -> Option<(Reg, Step, i64)> {
    if !plain(one) {
        return None;
    }
    let what = one.what.as_ref()?;
    let [Loc::Reg(dest)] = what.dests.as_slice() else {
        return None;
    };
    let register = |one: &Reg| one.width == dest.width && regs.integer(one.register);
    if ![2, 4].contains(&dest.width) || !register(dest) {
        return None;
    }
    let costs = &cpu.operations;
    let width = i64::from(dest.width);
    let (step, cost) = match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (Operation::Move, Some("mov"), [Loc::Reg(source)]) if register(source) && source.register != dest.register => {
            (Step::Copy(*source), costs.r#move)
        }
        (_, _, [first, ..]) if *first != Loc::Reg(*dest) => return None,
        (Operation::Binary, Some(name @ ("add" | "sub")), [_, Loc::Imm(Imm { value, address: None, .. })]) => {
            (Step::Add(if name == "add" { *value } else { -value }), costs.add)
        }
        (Operation::Unary, Some(name @ ("inc" | "dec")), [_]) => {
            (Step::Add(if name == "inc" { 1 } else { -1 }), costs.add)
        }
        (Operation::Binary, Some("shl" | "sal"), [_, Loc::Imm(Imm { value: count @ 1..=31, address: None, .. })]) => {
            (Step::Scale(1 << count), costs.shift)
        }
        (Operation::Binary, Some("add"), [_, Loc::Reg(other)]) if other == dest => (Step::Scale(2), costs.add),
        (Operation::Binary, Some("add"), [_, Loc::Reg(other)]) if register(other) => {
            (Step::AddRegister(*other), costs.add)
        }
        _ => return None,
    };
    Some((*dest, step, costs.sized(cost, width, i64::from(cpu.operand_bytes))))
}

/// A `lea` with no symbol that starts a chain: the register it makes, the
/// terms and displacement of its address, and what `cpu` charges for it.
pub fn lea_start(
    regs: Regs,
    one: &Insn,
    cpu: &Profile,
) -> Option<(Reg, Terms, i64, i64)> {
    if !plain(one) {
        return None;
    }
    let what = one.what.as_ref()?;
    let ([Loc::Reg(dest)], Operation::Address, Some("lea"), [Loc::Mem(cell)]) =
        (what.dests.as_slice(), what.op, what.name.as_deref(), what.sources.as_slice())
    else {
        return None;
    };
    let integer = |register: RegId| register != RegId::None && regs.integer(register) && !regs.is_stack(register);
    if ![2, 4].contains(&dest.width)
        || !regs.integer(dest.register)
        || cell.addr.is_some()
        || cell.selector.is_some()
        || !integer(cell.through)
        || !integer(cell.index_through)
    {
        return None;
    }
    let (base, index) = (regs.root(cell.through), regs.root(cell.index_through));
    let terms = if base == index { vec![(base, 1 + cell.scale)] } else { vec![(base, 1), (index, cell.scale)] };
    let width = i64::from(dest.width);
    Some((*dest, terms, cell.offset, cpu.operations.sized(cpu.operations.address, width, i64::from(cpu.operand_bytes))))
}

/// The 67h address naming `terms` plus `disp`, if one does.
/// `scales` are the index scales the target's 32-bit address form takes.
pub fn form(
    regs: Regs,
    terms: &[(RegId, i64)],
    disp: i64,
    scales: &BTreeSet<i64>,
) -> Option<AddressRef> {
    let at = |through: RegId, index: RegId, scale: i64| {
        (!regs.is_stack(index) && scales.contains(&scale)).then_some(AddressRef {
            through,
            index_through: index,
            scale,
            offset: disp,
            ..AddressRef::new(None)
        })
    };
    match *terms {
        [(only, 1)] => Some(AddressRef { through: only, offset: disp, ..AddressRef::new(None) }),
        [(only, scale)] => at(only, only, scale - 1).or_else(|| at(RegId::None, only, scale)),
        [(base, 1), (index, scale)] | [(index, scale), (base, 1)] => {
            at(base, index, scale).or_else(|| at(index, base, 1).filter(|_| scale == 1))
        }
        _ => None,
    }
}

/// The real-mode address naming `terms` plus `disp`, if one does: a lone base
/// or index register, or one of BX/BP and one of SI/DI. No prefix, no scale.
pub fn word_form(
    terms: &[(RegId, i64)],
    disp: i64,
) -> Option<AddressRef> {
    use llrm_x86::addressing16::{BASES, INDEXES};
    let word =
        |one: RegId| llrm_x86::registers::word_of(one).filter(|word| BASES.contains(word) || INDEXES.contains(word));
    let is_base = |one: RegId| BASES.contains(&one);
    match *terms {
        [(only, 1)] => Some(AddressRef { through: word(only)?, offset: disp, ..AddressRef::new(None) }),
        [(first, 1), (second, 1)] => {
            let (first, second) = (word(first)?, word(second)?);
            let (base, index) = if is_base(first) { (first, second) } else { (second, first) };
            (is_base(base) && !is_base(index)).then_some(AddressRef {
                through: base,
                index_through: index,
                scale: 1,
                offset: disp,
                ..AddressRef::new(None)
            })
        }
        _ => None,
    }
}
