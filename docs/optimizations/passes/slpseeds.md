> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpseeds.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpseeds.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpseeds.rs sha256:6da7c828dab1f5d916dcde265b08955ea611e861350c9293a53e905a57389501 -->

# slpseeds

`SLPVectorizer`'s roots that are no chain, phi or reduction: the operands
of the stores of a block, and the instructions read once.

## How it works
`vectorizeNonVectorizableInsts` (l.35156), pass 2: the stores of a block
whose value is an instruction (two at least) are roots whose value operands
are sorted by opcode and type, and each run of one opcode and type is given
to `tryToVectorizeSequence` (l.34805) and `tryToVectorizeList` (l.30044),
up to as many lanes as fill whole registers. The stores themselves stay
scalar and read the lanes by extracts: four `sdiv x, 2` of an FFT
butterfly, stored to four different arrays, are one `<4 x i64>` `sdiv`.

`vectorizeOnceUsedSeeds` (l.35884) is the very last attempt, after every
other root of the function is exhausted, block by block in post order. Its
seeds are the instructions of the block read by exactly one use
(`isOnceUsedSeed`: a binary operation that is not folded away by its
identity or absorbing constant, a unary one, a select, a cast that is not a
float to integer one, none of them the index of an address): such a value
needs one extract per lane, whatever the vector. `vectorizeSeeds` (l.35838)
sorts them by type, size, group (the opcode and type of
`generateKeySubkey`, an integer division by a variable on its own) and
program order, and `tryToVectorizeSequence` (l.34805) cuts the sorted list
into runs of one group, giving each, and the leftovers of runs too short to
fill a register, to `tryToVectorizeList` (l.30044) with `StandaloneSeeds`:
one register of lanes at most, no lane reading another, no window of
identical lanes. The tree is the bundle of the seeds, its users read the
lanes by extracts. The butterfly of an FFT, four `sdiv x, 2` stored to four
different arrays, is one `<4 x i64>` `sdiv` of the four sums.

DEVIATE SLP10 (owner arm64, exit 2026-12-21): the roots are the stores
alone (no return, call or atomic operand, no `ExtractsCost` gate, no
reduction pass); calls, `freeze` and `extractelement` are not once-used
seeds, and the poor-throughput seeds (`isPoorThroughputOp`) are taken with
them, not before them.

## Runs
In `slpvectorize`, after the store chains, the reductions and the compares:
the operands of the stores, then the once-used seeds.

## Used by
`slpvectorize`.

## Example
```text
%q0 = sdiv i64 %s0, 2        ; store %q0, ptr %a
%q1 = sdiv i64 %s1, 2        ; store %q1, ptr %b
        =>
%v  = sdiv <2 x i64> <%s0, %s1>, splat (i64 2)
%q0 = extractelement <2 x i64> %v, i64 0 ...
```
