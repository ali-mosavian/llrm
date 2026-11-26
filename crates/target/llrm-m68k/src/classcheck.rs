//! A register an instruction's operand class does not allow.
//!
//! The encoder writes a register's number whatever its bank, so a data-only
//! operand given `%a0` is assembled as `%d0`, while the text printed for it
//! reads right and gas fixes it (`sub.l %d4, %a0` became `suba.l`; the object
//! had `sub.l %d4, %d0`).
//!
//! The object writers refuse such an operand
//! ([`encode_final`](crate::encode::encode_final));
//! [`misplaced`] lists them in the final instructions.

use crate::compile::Item;
use crate::expand::is_address;
use crate::mc::{Inst, Operand};
use crate::mfunc::{Class, operand_class};

/// The first operand of `inst` whose register is in the other bank than its
/// class: `"SUB32dd operand 0 is in the other bank than its class"`.
pub fn wrong_bank(inst: &Inst) -> Option<String> {
    inst.operands
        .iter()
        .enumerate()
        .find_map(
            |(position, operand)| {
                let Operand::Reg(register) = operand else { return None };
                let wrong = match operand_class(inst.opcode, position) {
                    Some(Class::Dr8 | Class::Dr16 | Class::Dr32) => is_address(*register),
                    Some(Class::Ar32) => !is_address(*register),
                    _ => false,
                };
                wrong.then(|| format!("{} operand {position} is in the other bank than its class", inst.opcode))
            },
        )
}

/// [`wrong_bank`] of each instruction in `items`.
pub fn misplaced(items: &[Item]) -> Vec<String> {
    items.iter().filter_map(|item| if let Item::Inst(inst) = item { wrong_bank(inst) } else { None }).collect()
}
