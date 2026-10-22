//! SelectionDAG: value types, opcodes, the node arena and `getNode` with
//! CSE and constant folding (pin `SelectionDAG.cpp`).
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
//! # Runs
//! Nothing runs it yet: the builder (A1) and legalizer are the callers. The
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
//! # See also
//! LLVM's `SelectionDAG::getNode`
//! ([SelectionDAG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L8442)),
//! `FoldConstantArithmetic`
//! ([L7642](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L7642)),
//! `FoldSetCC`
//! ([L2698](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L2698)).

mod dag;
pub mod fold;
pub mod isd;
pub mod mvt;

pub use dag::{BooleanContents, DagConfig, NodeData, SDNode, SDNodeFlags, SDValue, SelectionDAG};
pub use isd::{CondCode, ISD};
pub use mvt::{EVT, MVT};

#[cfg(test)]
mod tests;
