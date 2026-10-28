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

/// `X86ISD::SETCC_CARRY flags` ([L6761][case]): the carry as all ones or zero,
/// `sbb r, r`. A CPU without `hasSBBDepBreaking` (all of these) takes the
/// pin's `getSBBZero`: `MOV32r0` twice, the flags copied to `EFLAGS`, and
/// `SBB32rr zero, zero`; under 32 bits the result is a sub-register of it.
///
/// [case]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L6761
pub fn select_setcc_carry<D: AddrDag>(
    dag: &mut D,
    node: u32,
) -> bool {
    let vt = dag.value_type(SDValue::new(node, 0));
    let sub = match vt {
        mvt::i32 => None,
        mvt::i16 => Some(x86::sub_16bit),
        mvt::i8 => Some(x86::sub_8bit),
        _ => return false,
    };
    let zero = SDValue::new(dag.machine_node(x86::MOV32r0, &[mvt::i32, mvt::i32], &[]), 0);
    let entry = dag.entry_node();
    let flags = dag.operand(node, 1);
    let copy = dag.copy_to_reg(entry, x86::EFLAGS, flags, None);
    let sbb = dag.machine_node(
        x86::SBB32rr,
        &[mvt::i32, mvt::i32],
        &[zero, zero, SDValue::new(copy, 0), SDValue::new(copy, 1)],
    );
    let mut result = SDValue::new(sbb, 0);
    if let Some(index) = sub {
        let index = dag.target_constant(i64::from(index), mvt::i32);
        result = SDValue::new(dag.machine_node(target_opcode::EXTRACT_SUBREG, &[vt], &[result, index]), 0);
    }
    dag.replace_uses(SDValue::new(node, 0), result);
    dag.remove_dead_node(node);
    true
}

/// `X86ISD::FIST chain, value, ptr` of an `i16` (DEVIATE SD215; owner sdag-b,
/// exit: promote it as the pin does, review 2026-12-10): `IST_Fp16m<float>`
/// with the address group, the value and the chain, as the table's `i32` and
/// `i64` patterns build theirs. False for another memory type.
pub fn select_fist16<D: AddrDag>(
    dag: &mut D,
    hooks: &crate::x86_hooks::X86Hooks,
    node: u32,
) -> bool {
    use crate::x86_addr::Matcher;
    if dag.memory_vt(node) != mvt::i16 {
        return false;
    }
    let (chain, value, ptr) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let opcode = match dag.value_type(value) {
        mvt::f32 => crate::x86_llrm::IST16_OPCODES[0],
        mvt::f64 => crate::x86_llrm::IST16_OPCODES[1],
        mvt::f80 => crate::x86_llrm::IST16_OPCODES[2],
        _ => return false,
    };
    let Some(a) = (Matcher {
        dag,
        pointer_type: hooks.options.pointer_type(),
        mode: hooks.options.address_mode,
        size: hooks.options.opt_for_size,
    })
    .select_addr_of(Some(node), ptr) else {
        return false;
    };
    let ops = [a.base, a.scale, a.index, a.disp, a.segment, value, chain];
    dag.morph_node_to(node, opcode, &[mvt::Other], &ops);
    true
}
