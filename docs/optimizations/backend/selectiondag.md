> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/selectiondag.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/selectiondag.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/selectiondag.rs sha256:9d2e5184d5aaaabf3a63930e925d69ae694e8a6240ea2da1cc74000f5759b0d4 -->

# selectiondag

SelectionDAG: value types, opcodes, the node arena, `getNode` with CSE and
constant folding (pin `SelectionDAG.cpp`), and the builder that makes one
DAG per block of MIR (pin `SelectionDAGBuilder.cpp`, scalar subset).

## How it works
Nodes live in an arena (`SelectionDAG`); a value is `SDValue { node,
res }`. `get_node` applies the pin's folds in its order (canonicalize
commutative operands, per-opcode identities, poison and undef operands,
`FoldConstantArithmetic`), then looks the node up by (opcode, types,
operands, data): equal requests give one node, with the flags both agree
on. Glue results are never shared. Constants hold their bits at the
node's width (up to 128).

The builder (`builder::build_block`) visits a block's instructions in order
and keeps the pin's chain bookkeeping: pending loads, pending exports,
`getRoot` / `getControlRoot`. A value used outside its block or by a phi
lives in a virtual register (`function_lowering::FunctionLoweringInfo`): the
block that defines it ends in `CopyToReg`, the blocks that read it start
from `CopyFromReg`; a phi is the `CopyToReg` of its incoming value in each
predecessor. `dump::normalized` makes a dump comparable with the pin's.
Arguments and returns go through the target's `Convention` (stack
parameters are loads of fixed objects, register ones live-in copies;
`ret` is `RET_GLUE`); a direct call is `callseq_start`, stack stores,
register copies, `X86ISD::CALL`, `callseq_end`, the result read back.
