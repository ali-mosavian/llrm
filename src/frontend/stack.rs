//! Port of `qbopt/frontend/stack.py`.
//!
//! Ported so far: `touches_sp`.

use iced_x86::Register;

use crate::frontend::declen::{Insn, WRITES, instruction_info_factory};

const _SP: [Register; 2] = [Register::SP, Register::ESP];

/// Whether this instruction could move the stack pointer by any means.
///
/// `stack_pointer_increment` reports 0 for `add sp,imm` and `leave`; only a
/// register-write check over sp/esp catches those too.
#[must_use]
pub fn touches_sp(insn: &Insn) -> bool {
    if insn.insn.stack_pointer_increment() != 0 {
        return true;
    }
    instruction_info_factory()
        .info(&insn.insn)
        .used_registers()
        .iter()
        .any(|used| _SP.contains(&used.register()) && WRITES.contains(&used.access()))
}

#[cfg(test)]
mod tests {
    use super::touches_sp;
    use crate::frontend::declen::decode;

    #[test]
    fn touches_sp_sees_writes_iced_gives_no_increment() {
        // Python: touches_sp is True for push ax, add sp,4 and leave; False for nop and mov ax,sp.
        for (bytes, expected) in [
            (&[0x50][..], true),
            (&[0x83, 0xC4, 0x04][..], true),
            (&[0xC9][..], true),
            (&[0x90][..], false),
            (&[0x89, 0xE0][..], false),
        ] {
            let insn = decode(bytes, 0).unwrap();
            assert_eq!(touches_sp(&insn), expected, "{bytes:02x?}");
        }
    }
}
