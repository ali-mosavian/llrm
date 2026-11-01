//! The node predicates the m68k tables name: llrm-dagisel's shared ones, then
//! m68k's own.

use llrm_dagisel::dag::{Dag, Hooks, LoadExt, SDValue};
pub use llrm_dagisel::predicates::*;

type Args<'a, D, H> = (&'a D, &'a H, SDValue);

/// `Mxloadi16` and `Mxloadi32`: an any-extending load passes when it is aligned
/// and NOT simple, as `M68kInstrInfo.td` writes it (`!LD->isSimple()`; x86's
/// `loadi16` has no `!`).
fn mx_loadi<D: Dag, H: Hooks<D>>(
    (dag, _, v): Args<D, H>,
    align: u64,
) -> bool {
    match dag.load_ext_type(v.node) {
        Some(LoadExt::NonExt) => true,
        Some(LoadExt::Ext) => dag.alignment(v.node) >= align && !dag.is_simple(v.node),
        _ => false,
    }
}

pub fn mx_loadi16<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    mx_loadi(args, 2)
}

pub fn mx_loadi32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    mx_loadi(args, 4)
}

/// `Mximm8_1to8` (also the 16- and 32-bit forms): `Imm >= 1 && Imm <= 8`.
pub fn mx_imm_1to8<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.constant(v).is_some_and(|imm| (1..=8).contains(&imm))
}

/// `isCallSeqNode`: `CALLSEQ_START`/`CALLSEQ_END`, or their lowered form.
fn is_call_seq_node<D: Dag>(
    dag: &D,
    node: u32,
) -> bool {
    use crate::dagisel::generated::ids::opcode;
    let code = dag.opcode(node);
    if dag.is_machine(node) {
        return code == opcode::ADJCALLSTACKDOWN || code == opcode::ADJCALLSTACKUP;
    }
    code == llrm_dagisel::generated::ids::isd::CALLSEQ_START || code == llrm_dagisel::generated::ids::isd::CALLSEQ_END
}

/// `getCallSeqChainInfo`: the nearest call-sequence node on a chain, or `Err`
/// when a token factor reaches more than one (or the walk gives up).
fn call_seq_chain<D: Dag>(
    dag: &D,
    chain: SDValue,
) -> Result<Option<u32>, ()> {
    use llrm_dagisel::generated::ids::{isd, mvt};
    let mut work = vec![chain];
    let mut visited = Vec::new();
    let mut found: Option<u32> = None;
    while let Some(value) = work.pop() {
        let node = value.node;
        if visited.contains(&node) {
            continue;
        }
        visited.push(node);
        if is_call_seq_node(dag, node) {
            match found {
                None => found = Some(node),
                Some(first) if first != node => return Err(()),
                Some(_) => {}
            }
        }
        let chains =
            (0..dag.num_operands(node)).map(|at| dag.operand(node, at)).filter(|op| dag.value_type(*op) == mvt::Other);
        if !dag.is_machine(node) && dag.opcode(node) == isd::TokenFactor {
            work.extend(chains);
            continue;
        }
        if let Some(next) = chains.into_iter().next() {
            // the walk keeps at most 8 branches, as the pin's does
            if work.len() == 8 {
                return Err(());
            }
            work.push(next);
        }
    }
    Ok(found)
}

/// `Mxstoreloadi*_safe`: `isSafeStoreLoad(N)`: a load stored at once is one
/// move unless either chain depends on a different call sequence.
///
/// DEVIATE from the pin, whose `Mxstoreloadi*_safe` fragments are an
/// `unindexedload` that checks no extension: a stored `zext i8 to i32` of a
/// load matches `MOV32jq` and copies four bytes (`llc` makes `move.l (buf,%pc),
/// (%a0)` of it). Only a load that does not extend is a move. Exit: the pin's
/// fragments check the extension type.
pub fn mx_store_load_safe<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    use llrm_dagisel::generated::ids::isd;
    if dag.opcode(v.node) != isd::STORE {
        return false;
    }
    let stored = dag.operand(v.node, 1);
    if dag.opcode(stored.node) != isd::LOAD || dag.load_ext_type(stored.node) != Some(LoadExt::NonExt) {
        return false;
    }
    let (Ok(load), Ok(store)) =
        (call_seq_chain(dag, dag.operand(stored.node, 0)), call_seq_chain(dag, dag.operand(v.node, 0)))
    else {
        return false;
    };
    match (load, store) {
        (None, None) => true,
        (Some(one), Some(two)) => one == two,
        _ => false,
    }
}
