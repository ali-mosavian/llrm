//! `WebAssemblyRegNumbering`: a wasm register number for each virtual
//! register that is read. Stackified registers count down from `i32::MIN`;
//! the rest take the next local after the parameters.
//!
//! # See also
//! `WebAssemblyRegNumbering.cpp`, pin d1106deb71cc.

use crate::mir::{Function, Operand, VReg, is_argument};

/// `UnusedReg`.
pub const UNUSED_REG: u32 = u32::MAX;

/// `MFI.getWAReg` for every virtual register.
pub fn reg_numbering(f: &Function) -> Vec<u32> {
    let mut numbers = vec![UNUSED_REG; f.num_vregs()];
    // Arguments share the local index space and take their own index.
    for inst in f.blocks[0].iter().take_while(|i| is_argument(i.opcode)) {
        if let [Operand::Def(reg), Operand::Imm(index)] = inst.operands[..] {
            numbers[reg as usize] = index as u32;
        }
    }
    let (mut stack, mut local) = (0_u32, f.params.len() as u32);
    for reg in 0..f.num_vregs() as VReg {
        if f.use_empty(reg) {
            continue;
        }
        if f.is_stackified(reg) {
            numbers[reg as usize] = i32::MIN as u32 | stack;
            stack += 1;
        } else if numbers[reg as usize] == UNUSED_REG {
            numbers[reg as usize] = local;
            local += 1;
        }
    }
    numbers
}
