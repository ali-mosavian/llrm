//! The registers and flags an instruction reads and writes, as `x86.instr`
//! states them: the rows the target's register file carries
//! (`Info::effects`), for the code of `bits` bits.

use llrm_lir::registers::Regs;
pub use llrm_x86::effects::{Effects, Served};

use crate::model::ir::Semantics;

/// What the table says of `what`, or None where it has no row (the instruction
/// is then decoded).
pub fn served(
    regs: Regs,
    bits: u32,
    what: &Semantics,
) -> Option<Served> {
    llrm_x86::effects::effects(&regs.effects, bits, what)
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::Regs;

    use super::*;
    use crate::model::ir::{Held, Loc, Mem, Operation};

    /// Compiles for two targets, taken turns in one thread, each ask the rows
    /// of their own register file: real mode has `les` and a flat target has
    /// none. Rows by code width kept in a registry answered by whichever
    /// target registered first.
    #[test]
    fn the_rows_are_those_of_the_register_file_asked() {
        let load = Semantics {
            name: Some("les".to_owned()),
            dests: vec![Loc::Held(Held { value: 1, width: 2 }), Loc::Held(Held { value: 2, width: 2 })],
            sources: vec![Loc::Mem(Mem::new(None, 4))],
            ..Semantics::new(Operation::Move)
        };
        for (info, bits, known) in [
            (&llrm_x86_m16::REGISTER_INFO, 16, true),
            (&llrm_x86_m32::REGISTER_INFO, 32, false),
            (&llrm_x86_m16::REGISTER_INFO, 16, true),
            (&llrm_x86_m32::REGISTER_INFO, 32, false),
        ] {
            assert_eq!(served(Regs(info), bits, &load).is_some(), known, "{bits} bits");
        }
    }
}
