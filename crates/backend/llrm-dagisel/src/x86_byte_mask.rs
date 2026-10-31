//! `(and $x, 0xff)` of a word that only addresses memory stays an `AND`.
//!
//! # How it works
//! The pin selects the mask as `MOVZX32rr8` of `sub_8bit` (X86InstrCompiler.td
//! L1777), which needs a/b/c/d; real-mode index registers (`si`, `di`, `bp`)
//! have no byte form, so the chain feeding the mask is built in a/b/c/d and
//! copied. `AND16ri` has no such constraint. When every user is an address
//! operand (or an `add`/`shl` that only feeds one) the mask becomes `AND16ri`;
//! a function optimized for size keeps the three-byte `MOVZX`.
//!
//! Numbered deviation: **SD330** the pin picks `MOVZX` for address uses too.
//! Owner w15. Exit: the allocator prices the copy. Review 2026-12-10.
//!
//! # Used by
//! [`crate::x86_select::select_node`].

use crate::dag::SDValue;
use crate::generated::ids::{isd, mvt, x86};
use crate::x86_addr::{AddrDag, AddrMode};
use crate::x86_hooks::X86Hooks;

/// Whether operand `at` of machine node `user` is the base or index of the
/// five-operand memory reference (base, scale, index, displacement, segment).
fn is_address_operand<D: AddrDag>(
    dag: &D,
    user: u32,
    at: usize,
) -> bool {
    dag.is_machine(user)
        && matches!(at, 0 | 2)
        && dag.num_operands(user) >= 5
        && dag.opcode(dag.operand(user, 1).node) == isd::TargetConstant
        && matches!(dag.constant(dag.operand(user, 1)), Some(1 | 2 | 4 | 8))
}

/// Whether every use of `value` is an address operand, looking through the
/// `add x,x` / `shl x,n` that scales an index real mode cannot scale.
fn only_addresses<D: AddrDag>(
    dag: &D,
    value: SDValue,
    through: bool,
) -> bool {
    let users = dag.users(value);
    !users.is_empty()
        && users.into_iter().all(|(user, at)| {
            if is_address_operand(dag, user, at) {
                return true;
            }
            through
                && dag.is_machine(user)
                && matches!(!dag.opcode(user), x86::ADD16rr | x86::SHL16ri)
                && only_addresses(dag, SDValue::new(user, 0), false)
        })
}

/// Selects `node`; false when the table's pattern should.
pub fn select_byte_mask<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    if hooks.options.address_mode != AddrMode::Real16
        || hooks.options.opt_for_size
        || dag.value_type(SDValue::new(node, 0)) != mvt::i16
        || dag.constant(dag.operand(node, 1)) != Some(0xff)
        || !only_addresses(dag, SDValue::new(node, 0), true)
    {
        return false;
    }
    let imm = dag.target_constant(0xff, mvt::i16);
    let new = dag.machine_node(x86::AND16ri, &[mvt::i16, mvt::i32], &[dag.operand(node, 0), imm]);
    dag.replace_uses(SDValue::new(node, 0), SDValue::new(new, 0));
    dag.remove_dead_node(node);
    true
}
