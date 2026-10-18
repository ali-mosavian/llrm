> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/constraintelim.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/constraintelim.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/constraintelim.rs sha256:b520d49a499518f9ea8ab0646c0bf9cca2480c1907c043564c18786892e61219 -->

# constraintelim

`constraint-elimination`: conditions the dominating branches, assumes and
memory accesses already decide become constants, by Fourier-Motzkin
elimination on a system of linear constraints over the values compared.

## How it works
A translation of LLVM `d1106deb71cc` `ConstraintElimination.cpp`, each
function linking the lines it translates. Two `ConstraintSystem`s
(unsigned and signed) hold the facts that hold at the point being visited;
the dominator tree is walked in the order of its DFS numbers, a fact
entering the systems when the walk enters the block it holds in and leaving
when the walk leaves it (`DFSInStack`). A compare is turned into a row by
`decompose` (an offset plus coefficients of values, looking through `add`,
`sub`, `mul` and `shl` by a constant, `zext`, `sext`, `trunc` and `gep`),
and decided by asking the system whether the row or its negation is
implied.

This file is the pass: the engine (`Decomposition`, `ConstraintTy`,
`ConstraintInfo`), the worklist over the dominator tree and the
`ConstraintElimination` function pass.

DEVIATE (anything MIR lacks):
1. A variable is an `Operand`: a value or a constant (a global, `null`),
   where LLVM's is a `Value *`.
2. A constant-expression GEP is not a `GEPOperator` here: only a `gep`
   instruction is decomposed (`decomposeGEP`); a constant one is stripped to
   its base or stays a variable.
3. `DebugCounter`, `DumpReproducers` (`generateReproducer`), the
   `LLVM_DEBUG` output and the debug-record rewrite of
   `replaceCmpWithConstant` are not translated.
4. `takeName` has no counterpart: the `usub.sat` and overflow replacements
   get fresh names.
5. Induction facts (`addBoundsForHeaderInductions`, `addInfoForInductions`,
   `getMonotonicityInfo`) are not translated: they need ScalarEvolution,
   which scev-port owns. Exit: join them once it lands. Measured effect:
   every file of the pass's tests that differs from the pinned `opt` is a
   loop one (`tools/llvmport/cecompare.py`).
6. Memory-access facts (`getConstraintFromMemoryAccess`) are not translated:
   they need `getBaseObjectSize`, which MIR lacks.

## Example
`if (x < 5) { if (x < 10) ... }`: the inner compare is decided true.

## Used by
Nothing yet: `difference` and `guards::Given::holds` move here.
