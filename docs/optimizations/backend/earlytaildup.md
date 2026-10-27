> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/earlytaildup.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/earlytaildup.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/earlytaildup.rs sha256:aa4079285156c5eedcd6eb83e613f064aa36d8cb2f03507644fbdf4f447449fc -->

# earlytaildup

Which blocks early tail duplication may copy, and into which predecessors:
`shouldTailDuplicate`, `isSimpleBB`, `canCompletelyDuplicateBB` and
`canTailDuplicate` of
[TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp)
with `PreRegAlloc` set and `LayoutMode` clear.

## How it works
A block is a candidate when it cannot fall through, is no single-block loop,
holds at most two instructions counting its jump (one at `-Os`; twenty if it
ends in an indirect jump), and none of them is a call, a return or not
duplicable. A block with more than 16 predecessors and more than 16
successors is out when it or a successor has a phi. Past those, an indirect
jump or a block of one jump is taken as it is; any other block only if every
predecessor goes to it alone, by a jump with no condition
(`canCompletelyDuplicateBB`).

```text
  b1: ... jmp b3        b1: ... <b3's instructions> jmp b4      (b3 has two
  b2: ... jmp b3   ==>  b2: ... <b3's instructions> jmp b4       instructions
  b3: x; jmp b4         b3 is gone when nothing else reaches it)  or fewer)
```

`tail_duplicate` is `tailDuplicate` for a tail with no phi whose values
nothing outside it reads: each predecessor that goes to it alone loses its
jump to it and takes a copy of its instructions (values the tail defines are
new in each copy), the successors' phis read from the predecessor what they
read from the tail, the tail's edge odds move to the predecessor, and the
tail goes when nothing reaches it.

DEVIATE ET1 (owner w-1, exit: step 3): a tail with a phi, or whose values
are read outside it, is left (`processPHI`, `updateSuccessorsPHIs`, the SSA
update). Nothing calls `tail_duplicate` yet.

DEVIATE ET3 (no number): LIR blocks end in explicit jumps, so the pin's
predecessor that falls through into the tail is the one that jumps to it
from the block just before it; it is left for last and takes the copy only
when it is the tail's one predecessor left (the pin's merge into the
previous block), which has the same result.

DEVIATE ET2 (no number): a block's instructions are not duplicable here if
they carry a group, a barrier or data (the placement's version adds a call's
record and a symbol, which only exist after allocation); the pin's
`isNotDuplicable` and `isConvergent` have no counterpart. LIR has no
`INLINEASM_BR`, EH pads or subregister phi operands, so those tests are not
made.

## Example
The unit tests: a join of two arms with one `mov` is a candidate;
the same block with a call, or with two, is not.

## Used by
`flow::machine_with`, first of the machine phases.
