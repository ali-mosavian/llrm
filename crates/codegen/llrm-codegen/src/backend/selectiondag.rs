//! SelectionDAG: value types, opcodes, the node arena, `getNode` with CSE and
//! constant folding (pin `SelectionDAG.cpp`), and the builder that makes one
//! DAG per block of MIR (pin `SelectionDAGBuilder.cpp`, scalar subset).
//!
//! # How it works
//! Nodes live in an arena ([`SelectionDAG`]); a value is `SDValue { node,
//! res }`. `get_node` applies the pin's folds in its order (canonicalize
//! commutative operands, per-opcode identities, poison and undef operands,
//! `FoldConstantArithmetic`), then looks the node up by (opcode, types,
//! operands, data): equal requests give one node, with the flags both agree
//! on. Glue results are never shared. Constants hold their bits at the
//! node's width (up to 128).
//!
//! The builder (`builder::build_block`) visits a block's instructions in order
//! and keeps the pin's chain bookkeeping: pending loads, pending exports,
//! `getRoot` / `getControlRoot`. A value used outside its block or by a phi
//! lives in a virtual register (`function_lowering::FunctionLoweringInfo`): the
//! block that defines it ends in `CopyToReg`, the blocks that read it start
//! from `CopyFromReg`; a phi is the `CopyToReg` of its incoming value in each
//! predecessor. `dump::normalized` makes a dump comparable with the pin's.
//! Arguments and returns go through the target's `Convention` (stack
//! parameters are loads of fixed objects, register ones live-in copies;
//! `ret` is `RET_GLUE`); a direct call is `callseq_start`, stack stores,
//! register copies, `X86ISD::CALL`, `callseq_end`, the result read back.
//! A `switch` of range clusters is the split-and-chain compare blocks of
//! `visitSwitch` (jump tables and bit tests included). Small constant copies
//! and fills are loads and stores; indirect calls, intrinsics, floats and
//! values wider than a register are refused (`Refused`) until their pieces
//! land.
//!
//! # Example
//! `%s = add i32 %a, %b` with `%a` and `%b` exported from an earlier block,
//! then `store i32 %s, ptr @g`:
//!
//! ```text
//! t2: i32,ch = CopyFromReg t0, Register:i32 %0
//! t4: i32,ch = CopyFromReg t0, Register:i32 %1
//! t5: i32 = add t2, t4
//! t6: ch = store t0, t5, GlobalAddress:i32<ptr @g> 0, poison:i32
//! ```
//!
//! # Runs
//! Nothing runs it yet but the tests and the shadow harness (sdag-d). The
//! old selector stays the default; the DAG path is `--isel=dag` (shadow).
//!
//! DEVIATE SD10: no `SDLoc`; debug locations and IR order are not carried.
//! Reason: nothing downstream reads them before the emitter (sdag-d).
//! Exit: sdag-d adds them if the emitter needs line info. Owner sdag-a.
//! Review 2026-11-13.
//!
//! DEVIATE SD11: no floating-point constant folding (`foldConstantFPMath`,
//! `FP_TO_SINT` of a constant), no FP constant
//! bitcasts; `ConstFp` holds raw bits. Reason: needs an `APFloat` (f80
//! included); not on the integer path A1 builds. Exit: with the FP builder
//! piece. Owner sdag-a. Review 2026-11-13.
//!
//! DEVIATE SD12: the folds that ask `ComputeNumSignBits`, `MaskedValueIsZero`
//! or `isGuaranteedNotToBeUndefOrPoison` (sext/zext of truncate, select with
//! an undef arm, `FREEZE`) are not made: nothing is dropped, the combiner
//! makes them. Exit: with `computeKnownBits` (sdag-b, B2). Owner sdag-a.
//! Review 2026-11-13.
//!
//! DEVIATE SD13: no vector types or nodes (`SPLAT_VECTOR`, `VSCALE`,
//! floating-point and 64-bit vectors, vector arithmetic), except the four
//! 128-bit integer vectors `v16i8`, `v8i16`, `v4i32`, `v2i64` of the arm64
//! flag `LLRM_ARM64_VECTORS`: their `BUILD_VECTOR`, `INSERT_VECTOR_ELT`,
//! `SCALAR_TO_VECTOR`, splat `VECTOR_SHUFFLE`, loads and stores exist since
//! 2026-11-10 (VEC1, VEC2). Exit: the rest, with the arm64 vectorizer's
//! pieces. Owner llrm-arm64-vec. Review 2026-11-12.
//!
//! DEVIATE SD15: `FindMergedConditions` is ported (`builder/mergedcond.rs`)
//! without the edge probabilities (`TProb`/`FProb`) the pin keeps on the blocks
//! it makes. Exit: probabilities kept by the DAG builder. Owner sdag-a.
//! Review 2026-11-13.
//!
//! DEVIATE SD16: no `LiveOutRegInfo`: a value copied out of a block reads
//! back without `AssertZext`/`AssertSext`. Exit: with `computeKnownBits`
//! (sdag-b). Owner sdag-a. Review 2026-11-13.
//!
//! DEVIATE SD17: virtual register numbers differ from the pin's, whose
//! selection of earlier blocks allocates registers between the exported
//! ones; comparisons renumber them. Exit: none. Owner sdag-a.
//!
//! DEVIATE SD18: `select` is never matched as `abs`/`smin`/`smax`/`umin`/
//! `umax` (`matchSelectPattern`). Exit: with the target action table (B0).
//! Owner sdag-a. Review 2026-11-13.
//!
//! DEVIATE SD100: a call's `TargetGlobalAddress` carries no relocation flag
//! (`[TF=7]`, PLT, in the pin): the object writer picks the relocation.
//! Exit: none. Owner sdag-a.
//!
//! DEVIATE SD101: `freeze` folds away only for a constant or another
//! `freeze` (`isGuaranteedNotToBeUndefOrPoison` also looks through
//! arithmetic on frozen operands, `Register`s of known width and the like).
//! Exit: with the known-bits translation (sdag-b). Owner sdag-a.
//!
//! DEVIATE SD102: an `invoke` has no `EH_LABEL`s around its call and a
//! `landingpad` builds nothing: the QB personality's pad is entered by the
//! runtime (`ehprepare`), with no unwind tables. Exit: none. Owner sdag-a.
//!
//! # See also
//! LLVM's `SelectionDAG::getNode`
//! ([SelectionDAG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L8442)),
//! `FoldConstantArithmetic`
//! ([L7642](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L7642)),
//! `FoldSetCC`
//! ([L2698](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L2698)).

pub use llrm_sdag::{actions, intrinsic_cost, isd, libcalls, lowering, mvt, target_names};
pub mod abs_combine;
pub mod address_analysis;
pub mod bitcast_combine;
pub mod bitreverse_combine;
pub mod bswap_combine;
pub mod builder;
pub mod chain_combine;
pub mod combine_andnot;
mod combine_avg;
pub mod combine_funnel;
pub mod combine_lifetime_end;
pub mod combine_load;
pub mod combine_load_combine;
pub mod combine_mulo;
pub mod combine_select_minmax;
pub mod combine_store_merge;
pub mod combine_token_factor;
pub mod combine_trunc_stores;
pub mod combine_urem_eq;
pub mod combine_vector;
pub mod ctcount_combine;
pub mod dag;
pub mod dag_combiner;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/dag_function.rs"]
pub mod dag_function;
pub mod dag_select;
pub mod dag_target;
pub mod dagisel_adapter;
mod demanded_bits;
pub mod div_combine;
pub mod div_const;
pub mod dump;
pub mod emit_api;
pub mod expand_divrem_const;
pub mod expand_fixed;
pub mod expand_funnel;
pub mod expand_overflow;
pub mod expand_sat;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/fastisel.rs"]
pub mod fastisel;
pub mod fold;
pub mod fp_combine;
pub mod fp_value;
pub mod function_lowering;
pub mod iminmax_combine;
pub mod indexed_combine;
pub mod inline_asm;
pub mod known_bits;
pub mod legalize_dag;
pub mod legalize_types;
pub mod libcall;
pub mod live_out;
pub mod logic_setcc;
mod match_rotate;
pub mod narrow_divrem;
pub mod opcodes;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/postprocess.rs"]
pub mod postprocess;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/preprocess.rs"]
pub mod preprocess;
mod reassociate;
pub mod reduce_load_width;
pub mod rotate_combine;
pub mod schedule;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/select.rs"]
pub mod select;
pub mod select_api;
pub mod setcc_extreme;
pub mod switchcg;
pub mod target_calling;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/unfold.rs"]
pub mod unfold;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/x86_carry.rs"]
pub mod x86_carry;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/x86_lowering.rs"]
pub mod x86_lowering;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/nodes.rs"]
pub mod x86_nodes;
#[cfg(test)]
#[path = "../../../../target/llrm-x86-backend/src/x86isd.rs"]
pub mod x86isd;

pub use actions::{Action, Features, LoadExt, TargetLowering};
pub use dag::{
    BooleanContents, DagConfig, MemBase, MemInfo, MemLocation, NodeData, Rauw, SDNode, SDNodeFlags, SDValue,
    SelectionDAG,
};
pub use isd::{CondCode, ISD};
pub use lowering::{TargetLoweringInfo, TypeAction};
pub use mvt::{EVT, MVT};

#[cfg(test)]
mod alloca_tests;
#[cfg(test)]
mod asmcall_tests;
#[cfg(test)]
mod bundle_tests;
#[cfg(test)]
mod codecall_tests;
#[cfg(test)]
mod combine_avg_tests;
#[cfg(test)]
mod combine_parity_tests;
#[cfg(test)]
mod dag_function_aggregates_tests;
#[cfg(test)]
mod dag_function_calls_tests;
#[cfg(test)]
mod dag_function_far_tests;
#[cfg(test)]
mod dag_function_flags_tests;
#[cfg(test)]
mod dag_function_float_tests;
#[cfg(test)]
mod dag_function_frame_tests;
#[cfg(test)]
mod dag_function_m64_tests;
#[cfg(test)]
mod dag_function_odds_tests;
#[cfg(test)]
mod dag_function_soft_float_tests;
#[cfg(test)]
mod dag_function_switch_tests;
#[cfg(test)]
mod dag_function_tail_tests;
#[cfg(test)]
mod dag_function_tests;
#[cfg(test)]
mod dag_select_tests;
#[cfg(test)]
mod dagisel_tests;
#[cfg(test)]
#[cfg(test)]
mod exact_shift_tests;
#[cfg(test)]
mod extload_tests;
#[cfg(test)]
mod fallthrough_tests;
#[cfg(test)]
mod farptr_tests;
#[cfg(test)]
mod fpret_tests;
#[cfg(test)]
mod fpround_tests;
#[cfg(test)]
mod freeze_tests;
#[cfg(test)]
mod gaps_tests;
#[cfg(test)]
mod infer_ptr_align_tests;
#[cfg(test)]
mod invoke_tests;
#[cfg(test)]
mod match_rotate_tests;
#[cfg(test)]
mod mathops_tests;
#[cfg(test)]
mod memconst_tests;
#[cfg(test)]
mod memlib_tests;
#[cfg(test)]
mod memspace_tests;
#[cfg(test)]
mod opcodes_tests;
#[cfg(test)]
mod or_as_add_tests;
#[cfg(test)]
mod overflow_tests;
#[cfg(test)]
mod schedule_tests;
#[cfg(test)]
mod switchbpi_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_ext;
#[cfg(test)]
mod tuning_tests;
#[cfg(test)]
mod vararg_tests;
