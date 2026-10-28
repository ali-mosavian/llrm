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

/// `Mxstoreloadi*_safe`: `isSafeStoreLoad(N)`.
///
/// DEVIATE: never safe, so a load and a store are never folded into one
/// memory-to-memory move. Reason: the pin's walk of call-sequence chains
/// (`getCallSeqChainInfo`) is not translated yet. Exit: M2 (DAGToDAG hooks).
pub fn mx_store_load_safe<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}
