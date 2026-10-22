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
//! Calls, returns, arguments and switches are refused (`Refused`) until A2.
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
//! DEVIATE SD15: `FindMergedConditions` (`a && b` branches split into two
//! branches) is not ported; the condition stays one `setcc`/`and`. Exit: the
//! switch-lowering piece (A2). Owner sdag-a. Review 2026-11-13.
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
//! # See also
//! LLVM's `SelectionDAG::getNode`
//! ([SelectionDAG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L8442)),
//! `FoldConstantArithmetic`
//! ([L7642](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L7642)),
//! `FoldSetCC`
//! ([L2698](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L2698)).

pub mod actions;
pub mod builder;
mod dag;
pub mod dump;
pub mod fold;
pub mod function_lowering;
pub mod isd;
pub mod legalize_types;
pub mod lowering;
pub mod mvt;
pub mod opcodes;
pub mod schedule;

pub use actions::{Action, Features, LoadExt, TargetLowering};
pub use dag::{BooleanContents, DagConfig, MemBase, MemInfo, NodeData, Rauw, SDNode, SDNodeFlags, SDValue, SelectionDAG};
pub use isd::{CondCode, ISD};
pub use lowering::{TargetLoweringInfo, TypeAction};
pub use mvt::{EVT, MVT};

#[cfg(test)]
mod opcodes_tests;
#[cfg(test)]
mod schedule_tests;
#[cfg(test)]
mod tests;
