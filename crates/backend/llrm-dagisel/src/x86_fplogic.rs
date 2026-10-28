//! `X86ISD::FAND`, `FOR` and `FXOR` of a scalar float against a pool mask: the
//! bitwise SSE instruction with the mask as its memory operand.
//!
//! # How it works
//! `LowerFABSorFNEG` and `LowerFCOPYSIGN` make these nodes for a `float` or
//! `double`; the pin lays the operand in a `v4f32`/`v2f64` first
//! (`SCALAR_TO_VECTOR`, then `EXTRACT_VECTOR_ELT`) so the table's vector
//! patterns select `ANDPSrm`/`ORPSrm`/`XORPSrm` with the 16-byte constant
//! folded. This is that selection with no vector node in between: the second
//! operand is the pool address, and the machine node reads it as a memory
//! reference.
//!
//! Numbered deviation: **SD108** `double` takes the `ps` form, as the only
//! bitwise rows `x86.instr` has are the `ps` ones (the pin selects `ANDPD`,
//! `ORPD`, `XORPD` for `v2f64`; the bits are the same). Owner sdag-a. Exit:
//! the `pd` rows. Review 2026-12-10.
//!
//! # Used by
//! [`crate::x86_select::select_node`].

use crate::dag::SDValue;
use crate::generated::ids::{mvt, x86, x86isd};
use crate::x86_addr::{AddrDag, Matcher};
use crate::x86_hooks::X86Hooks;

/// Selects `node`; false for an operand that is no address or a type that is no
/// scalar float.
pub fn select_fp_logic<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    let vt = dag.value_type(SDValue::new(node, 0));
    if vt != mvt::f32 && vt != mvt::f64 {
        return false;
    }
    let opcode = match dag.opcode(node) {
        x86isd::FAND => x86::ANDPSrm,
        x86isd::FOR => x86::ORPSrm,
        _ => x86::XORPSrm,
    };
    let (value, mask) = (dag.operand(node, 0), dag.operand(node, 1));
    let Some(a) = (Matcher {
        dag,
        pointer_type: hooks.options.pointer_type(),
        mode: hooks.options.address_mode,
        size: hooks.options.opt_for_size,
    })
    .select_addr(mask) else {
        return false;
    };
    let new = dag.machine_node(opcode, &[vt], &[value, a.base, a.scale, a.index, a.disp, a.segment]);
    dag.replace_uses(SDValue::new(node, 0), SDValue::new(new, 0));
    dag.remove_dead_node(node);
    true
}
