//! `X86ISD::LLRM_ZERO128` and `LLRM_COPY128`: sixteen bytes cleared or copied
//! with one SSE move.
//!
//! # How it works
//! The pin's `getOptimalMemOpType` makes a 16-byte piece of an inline
//! `memset`/`memcpy` a `v4i32` load and store (`MOVAPS`/`MOVUPS`), a zero one
//! `V_SET0` (`xorps`). This port has no vector `MVT` (SD540), so the builder
//! makes a chain-only node with the addresses and the alignment, and this
//! matcher turns it into the same machine nodes: `V_SET0` and a store, or a
//! load and a store. The aligned form is chosen at 16-byte alignment.
//!
//! # Used by
//! [`crate::x86_select::select_node`].

use crate::dag::SDValue;
use crate::generated::ids::{mvt, x86, x86isd};
use crate::x86_addr::{AddrDag, AddressOperands, Matcher};
use crate::x86_hooks::X86Hooks;

/// Selects `node` (`chain, destination[, source], alignment`); false for an
/// operand that is no address.
pub fn select_mem128<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    let copy = dag.opcode(node) == x86isd::LLRM_COPY128;
    let chain = dag.operand(node, 0);
    let (destination, source, alignment) = if copy {
        (dag.operand(node, 1), Some(dag.operand(node, 2)), dag.operand(node, 3))
    } else {
        (dag.operand(node, 1), None, dag.operand(node, 2))
    };
    let aligned = |dag: &D, value: SDValue| dag.constant(value).is_some_and(|align| align >= 16);
    let to = Matcher {
        dag,
        pointer_type: hooks.options.pointer_type(),
        mode: hooks.options.address_mode,
        size: hooks.options.opt_for_size,
    }
    .select_addr(destination);
    let Some(to) = to else { return false };
    let from = match source {
        Some(source) => {
            let found = Matcher {
                dag,
                pointer_type: hooks.options.pointer_type(),
                mode: hooks.options.address_mode,
                size: hooks.options.opt_for_size,
            }
            .select_addr(source);
            let Some(found) = found else { return false };
            Some(found)
        }
        None => None,
    };
    let (store, load) =
        if aligned(dag, alignment) { (x86::MOVAPSmr, x86::MOVAPSrm) } else { (x86::MOVUPSmr, x86::MOVUPSrm) };
    let (value, chain) = match from {
        Some(from) => {
            let read = dag.machine_node(
                load,
                &[mvt::v4i32, mvt::Other],
                &[from.base, from.scale, from.index, from.disp, from.segment, chain],
            );
            (SDValue::new(read, 0), SDValue::new(read, 1))
        }
        None => (SDValue::new(dag.machine_node(x86::V_SET0, &[mvt::v4i32], &[]), 0), chain),
    };
    let write = address_store(dag, store, &to, value, chain);
    dag.replace_uses(SDValue::new(node, 0), SDValue::new(write, 0));
    dag.remove_dead_node(node);
    true
}

fn address_store<D: AddrDag>(
    dag: &mut D,
    store: u16,
    to: &AddressOperands,
    value: SDValue,
    chain: SDValue,
) -> u32 {
    dag.machine_node(store, &[mvt::Other], &[to.base, to.scale, to.index, to.disp, to.segment, value, chain])
}
