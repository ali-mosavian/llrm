//! Where a parameter is while the function has not yet stored it into its home:
//! in the register the convention passed it in, from the entry until the
//! instruction that stores it. Read back from the code, like the frame rows.

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
            && full(one.memory_base()) == full(base)
            && one.memory_index() == RegId::None
            && i64::from(one.memory_displacement32() as i32) == wanted
            && one.op1_kind() == OpKind::Register
            && full(one.op1_register()) == full(register);
        if to_cell {
            return Some(one.ip() as usize + one.len());
        }
        if one.op0_kind() == OpKind::Register
            && full(one.op0_register()) == full(register)
            && one.mnemonic() != Mnemonic::Push
        {
            return None;
        }
        if let Cell::Stack { register: stack, .. } = cell {
            match one.mnemonic() {
                Mnemonic::Push | Mnemonic::Pop => depth += i64::from(one.stack_pointer_increment()),
                Mnemonic::Sub | Mnemonic::Add
                    if one.op0_kind() == OpKind::Register && full(one.op0_register()) == full(stack) =>
                {
                    if one.op1_kind() == OpKind::Register || one.op1_kind() == OpKind::Memory {
                        return None;
                    }
                    let amount = one.immediate(1) as i32 as i64;
                    depth += if one.mnemonic() == Mnemonic::Sub { -amount } else { amount };
                }
                _ if one.op0_kind() == OpKind::Register && full(one.op0_register()) == full(stack) => return None,
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
    use iced_x86::Register::{EAX, EBP, ECX, EDX, ESP};

    use super::{Cell, stored};

    fn framed(disp: i64) -> Cell {
        Cell::Frame { register: EBP, disp }
    }

    /// `push ebp; mov ebp, esp; sub esp, 8; mov [ebp-4], eax; mov [ebp-8], edx;
    /// ret`: eax is stored at 9, edx at 12, each after the one before; a
    /// register nothing stores, and one stored to another cell, are none.
    #[test]
    fn the_first_store_of_the_arrival_register_into_the_home_ends_its_stay_in_the_register() {
        let code = [0x55, 0x89, 0xE5, 0x83, 0xEC, 0x08, 0x89, 0x45, 0xFC, 0x89, 0x55, 0xF8, 0xC3];
        assert_eq!(stored(&code, framed(-4), EAX), Some(9));
        assert_eq!(stored(&code, framed(-8), EDX), Some(12));
        assert_eq!(stored(&code, framed(-8), EAX), None);
        assert_eq!(stored(&code, framed(-4), ECX), None);
    }

    /// A register written before its store holds something else by then: the
    /// store is no arrival's.
    #[test]
    fn a_register_overwritten_before_its_store_is_not_the_argument() {
        // mov eax, 1; mov [ebp-4], eax
        let code = [0xB8, 1, 0, 0, 0, 0x89, 0x45, 0xFC];
        assert_eq!(stored(&code, framed(-4), EAX), None);
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
        let stack = |from_cfa| Cell::Stack { register: ESP, entry: 4, from_cfa };
        assert_eq!(stored(&code, stack(-12), EAX), Some(8));
        assert_eq!(stored(&code, stack(-16), EDX), Some(11));
        assert_eq!(stored(&code, stack(-8), EAX), None);
    }
}
