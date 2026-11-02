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
//! `FP_TO_SINT` of a constant, `FoldSetCC` on FP constants), no FP constant
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
//! DEVIATE SD13: no vector types or nodes (`BUILD_VECTOR`, `SPLAT_VECTOR`,
//! `VSCALE`, ...). Reason: no vectors until 32-bit parity. Exit: with
//! vector MIR. Owner sdag-a. Review 2026-11-13.
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

pub use llrm_sdag::{actions, isd, lowering, mvt, x86isd};
pub mod abs_combine;
pub mod address_analysis;
pub mod bitcast_combine;
pub mod bitreverse_combine;
pub mod bswap_combine;
pub mod builder;
mod chain_combine;
pub mod combine_funnel;
pub mod combine_load;
pub mod combine_store_merge;
pub mod combine_token_factor;
pub mod ctcount_combine;
mod dag;
pub mod dag_combiner;
pub mod dag_function;
pub mod dag_select;
pub mod dag_target;
pub mod dagisel_adapter;
mod demanded_bits;
pub mod div_combine;
pub mod div_const;
pub mod dump;
pub mod emit;
pub mod expand_divrem_const;
pub mod expand_fixed;
pub mod expand_funnel;
pub mod expand_sat;
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
pub mod opcodes;
mod reassociate;
pub mod reduce_load_width;
pub mod rotate_combine;
pub mod schedule;
pub mod switchcg;
pub mod target_calling;
pub mod x86_carry;
pub mod x86_lowering;
pub mod x86_preprocess;

pub use actions::{Action, Features, LoadExt, TargetLowering};
pub use dag::{
    BooleanContents, DagConfig, MemBase, MemInfo, NodeData, Rauw, SDNode, SDNodeFlags, SDValue, SelectionDAG,
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
mod emit_tests;
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
