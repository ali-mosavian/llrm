//! `X86ISD::FCMP` without `fcomi` ([L6391][case]): the x87 compare, its status
//! word moved to `AX`, and `AH` into the flags.
//!
//! # How it works
//! `fucom` leaves the result in the FPU status word. `fnstsw ax` copies it to
//! `AX`, and `sahf` loads `AH` into `EFLAGS` as an unsigned compare. No CPU the
//! tables are for has `fcomi` (it came with the P6), so the pin's `canUseCMOV`
//! early exit never applies and the table's `UCOM_FpIr*` patterns (which
//! require `HasCMOV`) never do.
//!
//! The strict forms are not translated: the builder makes no strict
//! floating-point nodes.
//!
//! # Used by
//! [`crate::x86_select::select_node`].
//!
//! [case]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L6391

use crate::dag::SDValue;
use crate::generated::ids::{mvt, target_opcode, x86};
use crate::x86_addr::AddrDag;

/// Selects `node`; false for a compare type with no x87 form.
pub fn select_fcmp<D: AddrDag>(
    dag: &mut D,
    node: u32,
) -> bool {
    let (n0, n1) = (dag.operand(node, 0), dag.operand(node, 1));
    let cmp = match dag.value_type(n0) {
        mvt::f32 => x86::UCOM_Fpr32,
        mvt::f64 => x86::UCOM_Fpr64,
        mvt::f80 => x86::UCOM_Fpr80,
        _ => return false,
    };
    let glue = SDValue::new(dag.machine_node(cmp, &[mvt::Glue], &[n0, n1]), 0);
    let status = SDValue::new(dag.machine_node(x86::FNSTSW16r, &[mvt::i16], &[glue]), 0);
    let high = dag.target_constant(i64::from(x86::sub_8bit_hi), mvt::i32);
    let ah = SDValue::new(dag.machine_node(target_opcode::EXTRACT_SUBREG, &[mvt::i8], &[status, high]), 0);
    let entry = dag.entry_node();
    let copy = dag.copy_to_reg(entry, x86::AH, ah, None);
    let sahf = SDValue::new(dag.machine_node(x86::SAHF, &[mvt::i32], &[SDValue::new(copy, 1)]), 0);
    dag.replace_uses(SDValue::new(node, 0), sahf);
    dag.remove_dead_node(node);
    true
}

/// `X86ISD::POP_FROM_X87_REG chain, $fp0 [, glue]` ([L7069][case]): a call's
/// floating-point result is the copy from the x87 register; the node becomes a
/// `CopyFromReg` of the same operands.
///
/// [case]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L7069
pub fn select_pop_from_x87<D: AddrDag>(
    dag: &mut D,
    node: u32,
) {
    let (chain, register) = (dag.operand(node, 0), dag.operand(node, 1));
    let values = dag.num_values(node);
    let glue = (values == 3).then(|| dag.operand(node, 2));
    let vt = dag.value_type(SDValue::new(node, 0));
    let copy = dag.copy_from_reg_node(chain, register, vt, glue);
    for res in 0..values {
        dag.replace_uses(SDValue::new(node, res as u8), SDValue::new(copy, res as u8));
    }
    dag.remove_dead_node(node);
}
