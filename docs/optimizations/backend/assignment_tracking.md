> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/assignment_tracking.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/assignment_tracking.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/assignment_tracking.rs sha256:5f4b91a41427f6f174a806d5f7e88da4250e918afc905d7557bbea3502a6e9e5 -->

# assignment_tracking

`AssignmentTrackingAnalysis`: where each variable of a function lives, from
its `#dbg_assign` records and the stores they name, as a table of locations
the code generator emits.

## How it works
This file holds the result, `FunctionVarLocs`, and the table it is built
in, `FunctionVarLocsBuilder`: the variables (a variable and the bit range
of it a location covers), the variables that have one location for their
whole scope, and the locations that start just before an instruction. The
analysis that fills it comes in later commits; the first part of it is here,
the overlap map and the stores it reads.

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
