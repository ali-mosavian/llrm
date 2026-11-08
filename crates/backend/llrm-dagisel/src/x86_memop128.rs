//! `X86ISD::LLRM_ZERO128`, `LLRM_COPY128` and `LLRM_MOVE128`: sixteen bytes
//! cleared or copied with one SSE move.
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
            // A volatile piece is volatile on both accesses (`MachineMemOperand`).
            dag.set_node_mem_refs(read, &[node]);
            (SDValue::new(read, 0), SDValue::new(read, 1))
        }
        None => (SDValue::new(dag.machine_node(x86::V_SET0, &[mvt::v4i32], &[]), 0), chain),
    };
    let write = address_store(dag, store, &to, value, chain);
    dag.set_node_mem_refs(write, &[node]);
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

/// Selects `LLRM_MOVE128` (`chain`, then `destination, source, width,
/// alignment` per piece): every piece is loaded, then every piece is stored on
/// the loads' joined chain (`getMemmoveLoadsAndStores`: the ranges may
/// overlap). A 16-byte piece moves through `MOVUPS` (`MOVAPS` at 16-byte
/// alignment), a narrower one through the general register of its width. False
/// for an operand that is no address.
pub fn select_move128<D: AddrDag>(
    dag: &mut D,
    hooks: &X86Hooks,
    node: u32,
) -> bool {
    let chain = dag.operand(node, 0);
    let count = (dag.num_operands(node) - 1) / 4;
    let mut pieces = Vec::with_capacity(count);
    for piece in 0..count {
        let at = 1 + piece * 4;
        let (destination, source) = (dag.operand(node, at), dag.operand(node, at + 1));
        let (Some(width), Some(alignment)) =
            (dag.constant(dag.operand(node, at + 2)), dag.constant(dag.operand(node, at + 3)))
        else {
            return false;
        };
        let find = |dag: &mut D, address: SDValue| {
            Matcher {
                dag,
                pointer_type: hooks.options.pointer_type(),
                mode: hooks.options.address_mode,
                size: hooks.options.opt_for_size,
            }
            .select_addr(address)
        };
        let (Some(to), Some(from)) = (find(dag, destination), find(dag, source)) else { return false };
        let (load, store, vt) = match (width, alignment >= 16) {
            (16, true) => (x86::MOVAPSrm, x86::MOVAPSmr, mvt::v4i32),
            (16, false) => (x86::MOVUPSrm, x86::MOVUPSmr, mvt::v4i32),
            (8, _) => (x86::MOV64rm, x86::MOV64mr, mvt::i64),
            (4, _) => (x86::MOV32rm, x86::MOV32mr, mvt::i32),
            (2, _) => (x86::MOV16rm, x86::MOV16mr, mvt::i16),
            (1, _) => (x86::MOV8rm, x86::MOV8mr, mvt::i8),
            _ => return false,
        };
        pieces.push((to, from, load, store, vt));
    }
    let mut loaded = Vec::with_capacity(count);
    let mut chains = Vec::with_capacity(count);
    for (_, from, load, _, vt) in &pieces {
        let read = dag.machine_node(
            *load,
            &[*vt, mvt::Other],
            &[from.base, from.scale, from.index, from.disp, from.segment, chain],
        );
        loaded.push(SDValue::new(read, 0));
        chains.push(SDValue::new(read, 1));
    }
    let joined = if chains.len() == 1 { chains[0] } else { dag.token_factor(&chains) };
    let mut writes = Vec::with_capacity(count);
    for ((to, _, _, store, _), value) in pieces.iter().zip(loaded) {
        writes.push(SDValue::new(address_store(dag, *store, to, value, joined), 0));
    }
    let done = if writes.len() == 1 { writes[0] } else { dag.token_factor(&writes) };
    dag.replace_uses(SDValue::new(node, 0), done);
    dag.remove_dead_node(node);
    true
}
