> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slptree.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slptree.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slptree.rs sha256:495b33b9e1ad0c298b4be498be61e83bdcef1d826cd77078cf5f716bb7fb2b51 -->

# slptree

`SLPVectorizer`'s tree: a window of consecutive stores, and the loads,
arithmetic, casts, compares, selects and `llvm.fmuladd` calls that make
their values, become vector code when the cost model says the vectors are
cheaper.

## How it works
`BoUpSLP::buildTree` (l.12052 `buildTreeRec`) starts from the stores' values
and goes up the operands, one entry for each bundle of one lane per store:
the bundle of loads of consecutive cells, of one binary operation, one cast,
one compare, one select, one `fneg`, one `llvm.fmuladd`, or of extracts of
the lanes of one vector (that vector is the bundle); a bundle that is none
of these (constants, one value in every lane, anything else) is a gather,
the vector built from its scalars. A bundle that repeats a lane is made once
over the distinct lanes and read through a shuffle (`slpgather`). The
operands of a commutative operation are put in the order that makes the
bundles alike (`VLOperands::reorder`, `getShallowScore` l.1119).
`getTreeCost` (l.19795) adds each entry's vector cost less its scalar ones
(`getEntryCost` l.16168) and, for each scalar the tree does not use, the
extract of its lane; the tree is taken below the `slp-threshold` of 0.
`vectorizeTree` makes each bundle's vector after its last scalar, and the
vector store at the last store.

DEVIATE SLP1 (owner arm64, exit 2026-11-24): the tree has no reordered or
reversed loads, no alternate-opcode or gather-of-loads entries and
no operand look-ahead past the shallow score; the scale by the loop's trip
count is left out (it multiplies every entry alike, so the sign of the
total is the same). A scalar the tree uses
as a gather lane, or an external user placed before its bundle's last
scalar, makes the tree not form (the pin's scheduler may order it).

DEVIATE SLP3 (owner arm64, exit 2026-11-24): `BlockScheduling`'s dependency
graph is not built: a load that stands after one of the window's stores must
be one that `BasicAA`, scoped-noalias and TBAA say cannot read its bytes,
and nothing else between the first load and the last store may write memory,
nor read it after the first store. A load or store between the window's
stores that `BasicAA`, scoped-noalias and TBAA say cannot touch their bytes
may stay (`StoreSeparation`); a call may not.

## Runs
Under `slpvectorize`, on a window of stores whose values are not all
constants.

## Used by
`slpvectorize`.

## Example
Four `fmuladd`s of one scalar and four consecutive loads, plus a constant,
stored to four consecutive floats, become one vector chain.

```text
4 x (load c[i]; fmuladd a, c[i], 1.0; fadd 128.5; store out[i])
        =>
load <4 x float>; fmuladd.v4f32 (splat a, v, splat 1.0)
fadd <4 x float>; store <4 x float>
```
