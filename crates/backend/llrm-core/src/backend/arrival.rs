//! Finds how long a parameter stays in the register its convention passed it
//! in, from the machine code.
//!
//! # How it works
//! A parameter arrives in a register and the function stores it into its
//! frame home. Until that store the register, not the home, holds the value,
//! which the debug location of the parameter must say. `stored` decodes the
//! function's bytes from the entry along the straight line and returns the
//! offset after the first `mov [cell], register` that writes the home.
//!
//! It gives up (`None`) on the first branch, return or invalid code, on any
//! write to the register before the store (it no longer holds the argument),
//! and when nothing stores it. The home is a `Cell`: `Frame`, reached
//! through the frame register at a displacement, or `Stack`, where the code
//! keeps no frame register and the cell is `from_cfa` bytes from the
//! canonical frame address. For `Stack` the stack pointer's depth is tracked
//! through pushes, pops and constant `add`/`sub`, so `[esp+4]` after two
//! pushes is matched with the cell it names; any other write to the stack
//! pointer ends the search.
//!
//! ```text
//!  push ebp ; mov ebp,esp ; sub esp,8 ; mov [ebp-4],eax ; ...
//!  ^ entry                                                ^ eax is in the
//!    eax holds the argument                                 home from here
//! ```
//!
//! # Example
//! A function that stores `eax` and `edx` to its frame cells (`stored` returns
//! the offset after the store):
//!
//! ```text
//!  0  55            push ebp
//!  1  89 E5         mov  ebp,esp
//!  3  83 EC 08      sub  esp,8
//!  6  89 45 FC      mov  [ebp-4],eax      stored(cell ebp-4, eax) = Some(9)
//!  9  89 55 F8      mov  [ebp-8],edx      stored(cell ebp-8, edx) = Some(12)
//! 12  C3            ret                   stored(cell ebp-8, eax) = None
//! ```
//!
//! From unit test
//! `the_first_store_of_the_arrival_register_into_the_home_ends_its_stay_in_the_register`
//! in
//! `arrival.rs`.
//!
//! # Used by
//! `debuginfo.rs`, for 32-bit `-g` output: the parameter's location list is
//! the register up to the returned offset and the home after it.
//!
//! # See also
//! `cfi.rs`, which reads the same code for the frame rule.

use iced_x86::{Decoder, DecoderOptions, FlowControl, Mnemonic, OpKind};
use llrm_lir::registers::RegId;

fn full(register: RegId) -> RegId {
    if register.is_gpr() { register.full_register32() } else { register }
}

/// How a frame cell is reached: through the frame register, or, where the code
/// keeps none, through the stack pointer, the cell being `disp - bias` from the
/// canonical frame address (`FrameBase::Cfa`) and the stack pointer `entry`
/// bytes below it when the function starts.
#[derive(Clone, Copy)]
pub enum Cell {
    Frame { register: RegId, disp: i64 },
    Stack { register: RegId, entry: i64, from_cfa: i64 },
}

/// The offset after the first instruction that stores `register` into `cell`,
/// found along the straight line from the entry; None where a branch, a return
/// or a write to `register` comes first (the value is then not the argument any
/// more), or none stores it.
pub fn stored(
    code: &[u8],
    cell: Cell,
    register: RegId,
) -> Option<usize> {
    let mut decoder = Decoder::with_ip(32, code, 0, DecoderOptions::NONE);
    // The stack pointer's distance from the canonical frame address, where the
    // cell is addressed through it.
    let mut depth = match cell {
        Cell::Stack { entry, .. } => -entry,
        Cell::Frame { .. } => 0,
    };
    while decoder.can_decode() {
        let one = decoder.decode();
        if one.is_invalid() {
            return None;
        }
        let (base, wanted) = match cell {
            Cell::Frame { register, disp } => (register, disp),
            Cell::Stack { register, from_cfa, .. } => (register, from_cfa - depth),
        };
        let to_cell = one.mnemonic() == Mnemonic::Mov
            && one.op0_kind() == OpKind::Memory
            && full(RegId::from(one.memory_base())) == full(base)
            && one.memory_index() == (RegId::None).iced()
            && i64::from(one.memory_displacement32() as i32) == wanted
            && one.op1_kind() == OpKind::Register
            && full(RegId::from(one.op1_register())) == full(register);
        if to_cell {
            return Some(one.ip() as usize + one.len());
        }
        if one.op0_kind() == OpKind::Register
            && full(RegId::from(one.op0_register())) == full(register)
            && one.mnemonic() != Mnemonic::Push
        {
            return None;
        }
        if let Cell::Stack { register: stack, .. } = cell {
            match one.mnemonic() {
                Mnemonic::Push | Mnemonic::Pop => depth += i64::from(one.stack_pointer_increment()),
                Mnemonic::Sub | Mnemonic::Add
                    if one.op0_kind() == OpKind::Register && full(RegId::from(one.op0_register())) == full(stack) =>
                {
                    if one.op1_kind() == OpKind::Register || one.op1_kind() == OpKind::Memory {
                        return None;
                    }
                    let amount = one.immediate(1) as i32 as i64;
                    depth += if one.mnemonic() == Mnemonic::Sub { -amount } else { amount };
                }
                _ if one.op0_kind() == OpKind::Register && full(RegId::from(one.op0_register())) == full(stack) => {
                    return None;
                }
                _ => {}
            }
        }
        if !matches!(one.flow_control(), FlowControl::Next | FlowControl::Call) {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::RegId;
    const EAX: RegId = RegId::EAX;
    const EBP: RegId = RegId::EBP;
    const ECX: RegId = RegId::ECX;
    const EDX: RegId = RegId::EDX;
    const ESP: RegId = RegId::ESP;

    use super::{Cell, stored};

    fn framed(disp: i64) -> Cell {
        Cell::Frame { register: RegId::from(EBP), disp }
    }

    /// `push ebp; mov ebp, esp; sub esp, 8; mov [ebp-4], eax; mov [ebp-8], edx;
    /// ret`: eax is stored at 9, edx at 12, each after the one before; a
    /// register nothing stores, and one stored to another cell, are none.
    #[test]
    fn the_first_store_of_the_arrival_register_into_the_home_ends_its_stay_in_the_register() {
        let code = [0x55, 0x89, 0xE5, 0x83, 0xEC, 0x08, 0x89, 0x45, 0xFC, 0x89, 0x55, 0xF8, 0xC3];
        assert_eq!(stored(&code, framed(-4), RegId::from(EAX)), Some(9));
        assert_eq!(stored(&code, framed(-8), RegId::from(EDX)), Some(12));
        assert_eq!(stored(&code, framed(-8), RegId::from(EAX)), None);
        assert_eq!(stored(&code, framed(-4), RegId::from(ECX)), None);
    }

    /// A register written before its store holds something else by then: the
    /// store is no arrival's.
    #[test]
    fn a_register_overwritten_before_its_store_is_not_the_argument() {
        // mov eax, 1; mov [ebp-4], eax
        let code = [0xB8, 1, 0, 0, 0, 0x89, 0x45, 0xFC];
        assert_eq!(stored(&code, framed(-4), RegId::from(EAX)), None);
    }

    /// `push ebx; sub esp, 8; mov [esp+4], eax`: with no frame register the
    /// cell the frame register would have held at `-4` (the canonical frame
    /// address minus 12) is `esp+4` after 12 bytes, 4 of them the return
    /// address: the same store, found through the stack pointer and not
    /// through a register the code never sets. Reading the register's value
    /// as the frame register's would take `mov [esp-4], eax` for it, and the
    /// move of the stack pointer on the way (a push, a sub) is what tells
    /// the two apart.
    #[test]
    fn a_store_through_the_stack_pointer_is_found_where_the_code_keeps_no_frame_register() {
        // push ebx; sub esp, 8; mov [esp+4], eax; mov [esp+0], edx
        let code = [0x53, 0x83, 0xEC, 0x08, 0x89, 0x44, 0x24, 0x04, 0x89, 0x14, 0x24];
        // Entry: esp is 4 below the canonical frame address. After the push and
        // the sub: 16 below. The cell at bias 8, disp -4 is 12 below;
        // esp+4 is 12 below.
        let stack = |from_cfa| Cell::Stack { register: RegId::from(ESP), entry: 4, from_cfa };
        assert_eq!(stored(&code, stack(-12), RegId::from(EAX)), Some(8));
        assert_eq!(stored(&code, stack(-16), RegId::from(EDX)), Some(11));
        assert_eq!(stored(&code, stack(-8), RegId::from(EAX)), None);
    }
}
