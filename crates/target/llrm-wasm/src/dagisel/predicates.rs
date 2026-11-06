//! The node predicates the wasm tables name: llrm-dagisel's shared ones, then
//! WebAssembly's own (`WebAssemblyInstrInfo.td`, `WebAssemblyInstrSIMD.td`).

use llrm_dagisel::dag::{Dag, Hooks, SDValue};
use llrm_dagisel::generated::ids::mvt;
use llrm_dagisel::matcher::Vt;
pub use llrm_dagisel::predicates::*;

type Args<'a, D, H> = (&'a D, &'a H, SDValue);

/// `MVT::v8i8`, `v4i16`, `v2i32` of the pin's `MachineValueType.h`: the x86 ids
/// name no 64-bit vectors.
const V8I8: Vt = Vt(47);
const V4I16: Vt = Vt(58);
const V2I32: Vt = Vt(73);

/// `LaneIdx<N>`: `0 <= Imm && Imm < N`.
fn lane_idx<D: Dag, H: Hooks<D>>(
    (dag, _, v): Args<D, H>,
    lanes: i64,
) -> bool {
    dag.constant(v).is_some_and(|imm| (0..lanes).contains(&imm))
}

pub fn lane_idx_2<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    lane_idx(args, 2)
}

pub fn lane_idx_4<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    lane_idx(args, 4)
}

pub fn lane_idx_8<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    lane_idx(args, 8)
}

pub fn lane_idx_16<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    lane_idx(args, 16)
}

pub fn lane_idx_32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    lane_idx(args, 32)
}

fn memory_vt_is<D: Dag, H: Hooks<D>>(
    (dag, _, v): Args<D, H>,
    vt: Vt,
) -> bool {
    dag.memory_vt(v.node) == vt
}

pub fn memory_vt_f16<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::f16)
}

pub fn memory_vt_v8i8<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, V8I8)
}

pub fn memory_vt_v4i16<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, V4I16)
}

pub fn memory_vt_v2i32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, V2I32)
}

/// `bool_node`: `computeKnownBits(Op).countMinLeadingZeros() == 31`, that is
/// bits 31..1 known zero and bit 0 not.
pub fn bool_node<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.masked_value_is_zero(v, 0xFFFF_FFFE) && !dag.masked_value_is_zero(v, 1)
}

/// `add_nuw`: `N->getFlags().hasNoUnsignedWrap()`.
pub fn add_nuw<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.has_no_unsigned_wrap(v.node)
}
