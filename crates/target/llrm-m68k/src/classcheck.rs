//! A register an instruction's operand class does not allow.
//!
//! The encoder writes a register's number whatever its bank, so a data-only
//! operand given `%a0` is assembled as `%d0`, while the text printed for it
//! reads right and gas fixes it (`sub.l %d4, %a0` became `suba.l`; the object
//! had `sub.l %d4, %d0`).
//!
//! [`misplaced`] lists such operands in the final instructions.

use crate::compile::Item;
use crate::expand::is_address;
use crate::mc::Operand;
use crate::mfunc::{Class, operand_class};

/// The operands of `items` whose register is in the other bank than their
/// class: `"SUB32dd operand 0 is %a0's bank"`.
pub fn misplaced(items: &[Item]) -> Vec<String> {
    let mut found = Vec::new();
    for item in items {
        let Item::Inst(inst) = item else { continue };
        for (position, operand) in inst.operands.iter().enumerate() {
            let Operand::Reg(register) = operand else { continue };
            let wrong = match operand_class(inst.opcode, position) {
                Some(Class::Dr8 | Class::Dr16 | Class::Dr32) => is_address(*register),
                Some(Class::Ar32) => !is_address(*register),
                _ => false,
            };
            if wrong {
                found.push(format!("{} operand {position} is in the other bank than its class", inst.opcode));
            }
        }
    }
    found
}
