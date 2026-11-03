//! `(and x, 0xff)` and `(and x, 0xffff)` selected as an `AND` where the CPU
//! prices the extension above it.
//!
//! # How it works
//! The pin's table turns the mask into a `MOVZX` of the low byte or word
//! (X86InstrCompiler.td#L1773-L1777, `AddedComplexity = 1`) with no predicate:
//! its CPUs extend in one clock. The 486 and the Pentium take three for the
//! `movzx` and one for the `and`, so [`Options::mask_by_extension`], read from
//! the CPU's description (`Profile::masks_by_extension`: size, or an extension
//! no slower than the `and`), is false there at speed and this case builds the
//! `AND32ri` / `AND16ri` itself. The pin's pattern stays the default.
//!
//! Numbered deviation **SD460** (reason: the 486/P5 price of `movzx r32,r8` is
//! three clocks against one; owner x86-m32; exit: a predicate on the table's
//! movzx patterns when the generator takes one; review 2026-12-10).
//!
//! # Used by
//! [`crate::x86_select::select_node`].

use crate::dag::SDValue;
use crate::generated::ids::mvt;
use crate::x86_addr::AddrDag;
use crate::x86_hooks::X86Hooks;

/// `X86::AND16ri` and `X86::AND32ri`: the numbers of the generated tables
/// (`MACHINE_OPCODES`; a test reads them).
pub const AND16RI: u16 = 819;
pub const AND32RI: u16 = 860;

/// The `and` with an immediate for a byte or word mask of an `i32`, or the
/// byte mask of an `i16`; false for any other node, or where the CPU extends
/// as fast as it masks.
pub fn select_mask_and<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    if hooks.options.mask_by_extension {
        return false;
    }
    let vt = dag.value_type(SDValue::new(node, 0));
    let Some(mask) = dag.constant(dag.operand(node, 1)) else { return false };
    let opcode = match (vt, mask) {
        (mvt::i32, 0xff | 0xffff) => AND32RI,
        (mvt::i16, 0xff) => AND16RI,
        _ => return false,
    };
    let lhs = dag.operand(node, 0);
    let immediate = dag.target_constant(mask, vt);
    let and = dag.machine_node(opcode, &[vt, mvt::i32], &[lhs, immediate]);
    dag.replace_uses(SDValue::new(node, 0), SDValue::new(and, 0));
    dag.remove_dead_node(node);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::x86_i486;

    /// The numbers above are the tables' own.
    #[test]
    fn the_and_immediate_opcodes_are_the_tables() {
        let named = |number| x86_i486::MACHINE_OPCODES.iter().find(|(n, _)| *n == number).map(|(_, name)| *name);
        assert_eq!(named(AND16RI), Some("AND16ri"));
        assert_eq!(named(AND32RI), Some("AND32ri"));
    }
}
