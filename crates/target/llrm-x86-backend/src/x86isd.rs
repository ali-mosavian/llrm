//! Target opcodes (`X86ISD::*`) as [`ISD::Target`] codes, in the pin's order
//! of first use here; the legalizer and lowering add theirs after these.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::target_names::TargetNodeNames;

pub const CALL: ISD = ISD::Target(1);
pub const RET_GLUE: ISD = ISD::Target(2);
pub const REP_STOS: ISD = ISD::Target(3);
pub const REP_MOVS: ISD = ISD::Target(4);
/// A float call's result, popped from `st(0)`: `chain, Register $fp0, glue`.
pub const POP_FROM_X87_REG: ISD = ISD::Target(5);
/// A sibling call: `chain, callee, TargetConstant fpdiff, registers, regmask`;
/// the block's end (it returns by the callee's `ret`).
pub const TC_RETURN: ISD = ISD::Target(6);
/// A `llrm.ia16.asm.*` block (`tools/dagisel/own_ids.txt`); see
/// `selectiondag::inline_asm` in llrm-codegen.
pub const LLRM_ASM: ISD = ISD::Target(24578);
/// The selector of the segment a global lives in, an `i16`: a far global is
/// `build_pair(GlobalAddress, SEGMENT_OF)`. Carries `NodeData::Global`.
pub const SEGMENT_OF: ISD = ISD::Target(24580);
/// `llrm.ia16.window`: a huge pointer as the far one into its current
/// segment window, `far -> far` (not a delta-0 `HUGE_ADD`, which does not
/// normalize; expanded by the type legalizer).
pub const LLRM_WINDOW: ISD = ISD::Target(24581);
/// A `llrm.ia16.code.*` block: `chain, TargetExternalSymbol(name),
/// TargetFrameIndex per place the name patches in, glue`; the result is read
/// from the convention's result registers after it.
pub const LLRM_CODE: ISD = ISD::Target(24582);
/// `llrm.ia16.ptrdiff`: the byte difference of two huge pointers into one
/// object, `(far, far) -> i32`, expanded by the type legalizer with the
/// machine's selector stride.
pub const LLRM_PTRDIFF: ISD = ISD::Target(24583);
/// A call through a far pointer: `X86ISD::CALL` with the callee's offset and
/// selector in a 4-byte stack cell, `chain, TargetFrameIndex cell, ...` as
/// `CALL`'s other operands; printed `call far [cell]`.
///
/// DEVIATE SD208: the pin has no far call. The old selector stored the offset
/// then the selector to a 4-byte temporary and did `call m16:16` through it.
/// Exit: none (m16 only). Owner sdag-b. Review 2026-12-10.
pub const LLRM_FARCALL: ISD = ISD::Target(24584);

/// `TC_RETURN` at m16: the pin has no pattern for it there. Same operands.
/// DEVIATE: a far callee is jumped to after the epilogue. Owner sdag-a (w9).
pub const LLRM_TC_RETURN: ISD = ISD::Target(24587);

/// A read from or a write to an I/O port (`llrm.ia16.in`/`out`): `chain, port`
/// giving the value and the chain, or `chain, port, value` giving the chain.
/// DEVIATE: the pin has no port intrinsics; the old selector made `in`/`out`
/// barriers. Owner sdag-a (w9).
pub const LLRM_PORT: ISD = ISD::Target(24586);
/// The flags pushed for the `iret` of an interrupt function a call enters:
/// `chain`, giving the chain; no register is read or written.
/// DEVIATE: the pin never calls an interrupt handler. Owner x86-m16.
pub const LLRM_PUSHF: ISD = ISD::Target(24588);
/// 16 zero bytes stored at an address: `chain, pointer`, giving the chain.
/// Selected as `V_SET0` and a 16-byte store; no vector or `i128` value is made.
///
/// DEVIATE SD540: the pin stores a `v4i32` zero (`getOptimalMemOpType`); this
/// port has no vector `MVT` (SD13). Exit: vector `MVT`s land. Owner m64.
pub const LLRM_ZERO128: ISD = ISD::Target(24591);
/// 16 bytes copied between two addresses: `chain, destination, source`, giving
/// the chain. Selected as a 16-byte load and store. Same deviation as
/// [`LLRM_ZERO128`].
pub const LLRM_COPY128: ISD = ISD::Target(24592);
/// `llrm.x86.udiv64by32`: `hi, lo, divisor` (all `i32`), giving the quotient
/// and the remainder of `hi:lo / divisor`. Selected as `DIV32r` with `hi` in
/// `EDX` and `lo` in `EAX`; a quotient over 32 bits is the hardware's `#DE`.
///
/// DEVIATE DV1: the pin has no such builtin (libgcc divides this way with
/// `divl`). Exit 2026-11-12: none planned; m68k `divu.l` takes the same
/// shape. Owner m32.
pub const LLRM_UDIV64BY32: ISD = ISD::Target(24593);
/// Inline `memmove` with 16-byte pieces: `chain`, then per piece `destination,
/// source, width, alignment`; every piece is loaded before any is stored, a
/// 16-byte one with `MOVUPS`, giving the chain. DEVIATE SD540, as
/// [`LLRM_COPY128`]: the pin loads `v4i32`. Exit: vector `MVT`s land. Owner
/// m64.
pub const LLRM_MOVE128: ISD = ISD::Target(24594);
/// `X86ISD::VASTART_SAVE_XMM_REGS` (the pin's number): `chain, al, frame index
/// of the register save area, offset of the first vector register's slot`,
/// the vector argument registers not taken by named parameters stored there
/// when `al` is not zero.
pub const VASTART_SAVE_XMM_REGS: ISD = ISD::Target(783);
/// A QB runtime routine's answer, which is the flags as `cmp answer, 0` leaves
/// them: `flags` (the `EFLAGS` copy after the call) -> the answer. Only a
/// compare with zero reads it, and `EmitCmp` returns the flags operand; no
/// node of this kind is selected.
///
/// DEVIATE SD310: the pin's calls answer in registers. Exit: none (QB only).
/// Owner sdag-c. Review 2026-12-10.
pub const LLRM_CALL_FLAGS: ISD = ISD::Target(24585);

/// The name of a private `X86ISD` code (`getTargetNodeName`), or none for a
/// code that is the pin's own number.
pub fn private_name(code: u16) -> Option<&'static str> {
    Some(match code {
        1 => "X86ISD::CALL",
        2 => "X86ISD::RET_GLUE",
        3 => "X86ISD::REP_STOS",
        4 => "X86ISD::REP_MOVS",
        5 => "X86ISD::POP_FROM_X87_REG",
        6 => "X86ISD::TC_RETURN",
        24578 => "X86ISD::LLRM_ASM",
        24580 => "X86ISD::SEGMENT_OF",
        24581 => "X86ISD::LLRM_WINDOW",
        24582 => "X86ISD::LLRM_CODE",
        24584 => "X86ISD::LLRM_FARCALL",
        24586 => "X86ISD::LLRM_PORT",
        24588 => "X86ISD::LLRM_PUSHF",
        24587 => "X86ISD::LLRM_TC_RETURN",
        24585 => "X86ISD::LLRM_CALL_FLAGS",
        24583 => "X86ISD::LLRM_PTRDIFF",
        24591 => "X86ISD::LLRM_ZERO128",
        24592 => "X86ISD::LLRM_COPY128",
        24593 => "X86ISD::LLRM_UDIV64BY32",
        24594 => "X86ISD::LLRM_MOVE128",
        _ => return None,
    })
}

/// `getTargetNodeName`.
pub fn name(code: u16) -> String {
    private_name(code).map_or_else(|| format!("X86ISD#{code}"), str::to_owned)
}

/// x86's names and pin numbers for the shared dump and adapter.
pub struct X86Names;

pub static NAMES: X86Names = X86Names;

impl TargetNodeNames for X86Names {
    fn name(
        &self,
        code: u16,
    ) -> Option<&'static str> {
        private_name(code)
    }

    fn pin_id(
        &self,
        code: u16,
    ) -> Option<u16> {
        let name = private_name(code)?.strip_prefix("X86ISD::")?;
        Some(llrm_dagisel::generated::ids::x86isd_id(name).unwrap_or(u16::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Moving the names out of shared code must keep the dump text and the
    /// pin numbers: `CALL` prints `X86ISD::CALL` and maps to the pin's id.
    #[test]
    fn test_names_and_pin_ids_survive_the_move() {
        assert_eq!(NAMES.name(1), Some("X86ISD::CALL"));
        assert_eq!(NAMES.pin_id(1), llrm_dagisel::generated::ids::x86isd_id("CALL"));
        assert_eq!(NAMES.pin_id(9999), None);
    }
}
