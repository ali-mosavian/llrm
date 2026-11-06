> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/assignment_tracking.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/assignment_tracking.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/assignment_tracking.rs sha256:d8fc3d7d919c3ceb131a6d15574a98bc15a7f571d8a79a8f1d909bd011afa385 -->

# assignment_tracking

`AssignmentTrackingAnalysis`: where each variable of a function lives, from
its `#dbg_assign` records and the stores they name, as a table of locations
the code generator emits.

## How it works
`FunctionVarLocs` is the result, built in `FunctionVarLocsBuilder`:
the variables (a variable and the bit range of it a location covers), the
variables that have one location for their whole scope, and the locations
that start just before an instruction. `build_overlap_map` finds which
fragments contain which and which stores write them; `Lowering` is the
dataflow: a state per block (`BlockInfo`), transfer functions for stores,
escaping calls and records, and `run`, the fixpoint over the blocks.

## Example
A variable `x` kept in `%a` from its first use, and `y` in its stack slot
for the whole function:

```text
=== Variables ===
[1] x
[2] y
=== Single location vars ===
DEF Var=[2] Expr=!DIExpression(DW_OP_deref) Values=(%slot )
=== In-line variable defs ===
DEF Var=[1] Expr=!DIExpression() Values=(%a )
```

## Complexity
Linear in the locations: a variable is found by a hash of its key, a wedge
by its instruction. The analysis that builds it keeps the pin's bound (a
fixpoint over blocks and variables, cut off at `MAX_BLOCKS`).

## See also
`llvm/lib/CodeGen/AssignmentTrackingAnalysis.cpp`, pin d1106deb71cc.
