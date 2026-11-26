// SPDX-License-Identifier: GPL-3.0-or-later
//! `addq.w #1,(d,An,Xn)`, `or.b %d0,(%a0)`: a read-modify-write of memory with
//! an immediate or a register.
//!
//! # How it works
//! [`select`] turns `store (add (load a), imm), a` and its `sub` into the
//! memory-destination `ADDnpi`/`ADDnfi` (and `SUB`) rows of the pin's table,
//! when `a` is an `(d16,An)` or `(d8,An,Xn)` address and the load and the
//! store follow each other on the chain with nothing else using the loaded
//! value. 10 bytes (`moveq #1,%d1; add.w (0,%a0,%d0),%d1; move.w
//! %d1,(0,%a0,%d0)`) become 4.
//!
//! A register source (`store (add|sub|and|or (load a), reg), a`, any of the
//! three address modes) is the memory-destination `ADDnjd`/`ADDnpd`/`ADDnfd`
//! rows (and `SUB`, `AND`, `OR`) of the pin's table, which no pattern uses:
//! `moveb (%a0),%d0; orb %d1,%d0; moveb %d0,(%a0)` (6 bytes) becomes `orb
//! %d1,(%a0)` (2). An immediate `and` or `or` takes the `i` rows the same way.
//!
//! Translation: gcc's memory-destination `addhi3`/`addsi3` and `addqsi3`
//! patterns (`config/m68k/m68k.md`), and
//! `X86DAGToDAGISel::foldLoadStoreIntoMemOperand`, guarded as
//! `isFusableLoadOpStorePattern`.
//!
//! DEVIATE M68K-RMW-MODES: the pin has the `store (add (load a), imm), a`
//! patterns for `(An)` only (`M68kInstrArithmetic.td`, "add imm, (An)"), the
//! other modes' rows go unused, and it has none with a register source or for
//! `and`/`or`. Reason: gcc selects `addq` on every mode
//! (`++counts[x]`, histo -Os 164 B against 162). Owner w8. Exit: 2026-11-24, or
//! when the pin has the patterns.

use llrm_codegen::backend::selectiondag::SDValue as Value;
use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{MemInfo, NodeData};
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::interp::Outcome;

use super::addr;
use super::opcodes::code_of;

/// The row for `op` (`ADD` or `SUB`) of `bytes` on the address mode `letter`.
fn row(
    op: &str,
    bytes: u32,
    letter: char,
) -> Option<u16> {
    code_of(&format!("{op}{}{letter}i", bytes * 8))
}

/// The other operand of the read-modify-write: an immediate, or a register.
enum Source {
    Constant(i64),
    Register(SDValue),
}

/// `store (add|sub|and|or (load a), imm|reg), a` of a byte, word or long: the
/// store, the loaded value's address, the opcode and the other operand. The
/// loaded value is either operand of `add`, `and` and `or`.
fn read_modify_write(
    adapter: &DagisAdapter<'_>,
    node: u32,
) -> Option<(MemInfo, SDValue, &'static str, Source, Vec<SDValue>)> {
    let store = Value { node, res: 0 };
    if adapter.dag.opcode(store) != ISD::Store {
        return None;
    }
    let NodeData::Mem(info) = adapter.dag.node(store).data else { return None };
    if info.volatile || info.ordering.is_some() || !matches!(info.mem_vt, EVT::I8 | EVT::I16 | EVT::I32) {
        return None;
    }
    // (chain, value, address)
    let value = Dag::operand(adapter, node, 1);
    let address = Dag::operand(adapter, node, 2);
    let op = match adapter.dag.opcode(Value { node: value.node, res: value.res }) {
        ISD::Add => "ADD",
        ISD::Sub => "SUB",
        ISD::And => "AND",
        ISD::Or => "OR",
        _ => return None,
    };
    if adapter.dag.value_type(Value { node: value.node, res: 0 }) != info.mem_vt {
        return None;
    }
    // `isFusableLoadOpStorePattern`'s `StoredVal.hasOneUse()`: the operation
    // is gone once it is in the store, and another user would compute it again
    // from the memory the store already changed
    if !Dag::has_n_uses_of_value(adapter, value, 1) {
        return None;
    }
    let (first, second) = (Dag::operand(adapter, value.node, 0), Dag::operand(adapter, value.node, 1));
    let is_load = |one: SDValue| adapter.dag.opcode(Value { node: one.node, res: 0 }) == ISD::Load;
    let (loaded, other) = if is_load(first) {
        (first, second)
    } else if op != "SUB" && is_load(second) {
        (second, first)
    } else {
        return None;
    };
    let source = match Dag::constant(adapter, other) {
        Some(constant) => Source::Constant(constant as i64),
        None if other.node != loaded.node => Source::Register(other),
        None => return None,
    };
    let load = Value { node: loaded.node, res: 0 };
    let NodeData::Mem(loading) = adapter.dag.node(load).data else { return None };
    if loading.mem_vt != info.mem_vt || loading.ext.is_some() || loading.volatile || loading.ordering.is_some() {
        return None;
    }
    // the same address, and nothing else reading the loaded value or its
    // chain
    let load_address = Dag::operand(adapter, loaded.node, 1);
    if load_address != address || loaded.res != 0 {
        return None;
    }
    if !Dag::has_n_uses_of_value(adapter, SDValue { node: loaded.node, res: 0 }, 1)
        || !Dag::has_n_uses_of_value(adapter, SDValue { node: loaded.node, res: 1 }, 1)
    {
        return None;
    }
    // the load's chain feeds the store directly, or through the token factor
    // of the loads pending before it (`foldLoadStoreIntoMemOperand`)
    let chain = Dag::operand(adapter, node, 0);
    let load_chain = Dag::operand(adapter, loaded.node, 0);
    let others: Vec<SDValue> = if chain.node == loaded.node && chain.res == 1 {
        Vec::new()
    } else if adapter.dag.opcode(Value { node: chain.node, res: 0 }) == ISD::TokenFactor
        && Dag::has_n_uses_of_value(adapter, SDValue { node: chain.node, res: 0 }, 1)
    {
        let all: Vec<SDValue> =
            (0..adapter.num_operands(chain.node)).map(|at| Dag::operand(adapter, chain.node, at)).collect();
        if !all.contains(&SDValue { node: loaded.node, res: 1 }) {
            return None;
        }
        all.into_iter().filter(|one| *one != SDValue { node: loaded.node, res: 1 }).collect()
    } else {
        return None;
    };
    let mut before = others;
    before.push(load_chain);
    Some((info, address, op, source, before))
}

/// A read-modify-write of memory as one memory-destination instruction: with an
/// immediate, an `add` or `sub` on a displaced or indexed address (`(An)` is
/// the pin's own pattern), an `and` or `or` on any; with a register, any.
pub fn select(
    adapter: &mut DagisAdapter<'_>,
    node: u32,
) -> Option<Outcome> {
    let (info, address, op, source, chains) = read_modify_write(adapter, node)?;
    let bytes = match info.mem_vt {
        EVT::I8 => 1,
        EVT::I16 => 2,
        _ => 4,
    };
    let pin_has_it = matches!(source, Source::Constant(_)) && matches!(op, "ADD" | "SUB");
    if pin_has_it && addr::select_ari(adapter, Some(node), address).is_some() {
        return None;
    }
    let (letter, parts): (char, Vec<SDValue>) = if let Some(parts) = addr::select_ari(adapter, Some(node), address) {
        ('j', vec![parts])
    } else if let Some(parts) = addr::select_arid(adapter, Some(node), address) {
        ('p', parts.to_vec())
    } else if let Some(parts) = addr::select_arii(adapter, Some(node), address) {
        ('f', parts.to_vec())
    } else {
        return None;
    };
    let width = bytes * 8;
    let (selected, operand) = match source {
        Source::Register(register) => (code_of(&format!("{op}{width}{letter}d"))?, register),
        Source::Constant(constant) => {
            // a negative constant of 1 to 8 is the quick form of the opposite
            // operation
            let signed = (constant << (64 - width)) >> (64 - width);
            let (op, constant) = match (op, signed) {
                ("ADD", -8..=-1) => ("SUB", -signed),
                ("SUB", -8..=-1) => ("ADD", -signed),
                _ => (op, signed),
            };
            let vt = adapter.value_type(SDValue { node: Dag::operand(adapter, node, 1).node, res: 0 });
            (row(op, bytes, letter)?, adapter.target_constant(constant, vt))
        }
    };
    // the load goes with the operation: the store takes the chains that
    // preceded it
    let before = if chains.len() == 1 { chains[0] } else { Dag::token_factor(adapter, &chains) };
    let mut operands = parts;
    operands.push(operand);
    operands.push(before);
    let results = vec![adapter.value_type(SDValue { node, res: 0 })];
    super::local::morph(adapter, node, selected, &results, &operands);
    Some(Outcome::Selected)
}
