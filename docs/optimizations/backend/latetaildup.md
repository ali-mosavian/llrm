> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/latetaildup.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/latetaildup.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/latetaildup.rs sha256:1ac3cb3b474c3856e48cf56497386006a00fc4ebb53e90aa19a89eda0f01470c -->

# latetaildup

The stand-alone tail duplication after register allocation, before block
placement: `tailDuplicateBlocks` with `PreRegAlloc` clear and `LayoutMode`
clear, of
[TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L274).

## How it works
A block is a tail when it cannot fall through, is no single-block loop and
holds at most two instructions counting its jump (one at `-Os`)
([`shouldTailDuplicate`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L562)).
After allocation nothing else is asked of it: there are no phis to
update, no value to rename (the registers are the machine's), and
`canCompletelyDuplicateBB` is not consulted. Each predecessor that goes to
it by a jump with no condition takes its instructions in place of the jump;
the tail goes when nothing reaches it. A block of one jump is bypassed
(`duplicateSimpleBB`).

```text
  b1: ... jmp b3        b1: ... <b3's instructions> jmp b4
  b2: ... jmp b3   ==>  b2: ... <b3's instructions> jmp b4
  b3: x; jmp b4         (b3 goes when nothing else reaches it)
```

DEVIATE TL1 (owner greedy-subreg, exit 2026-11-27): a tail with a call, a
call's record, a group, a symbol, bytes spread over others, an indirect jump
or a sibling call is not copied. The pin copies a call or a return after
allocation; here a copy keeps no callee (the callee is named by the
instruction's id) and claims no source bytes of its own (placement's
`copied` refuses the same).

DEVIATE TL3 (owner greedy-subreg, exit 2026-11-20): the pin's return block
already holds the frame's teardown, so a framed function's is longer than
the bound and is never copied; here the teardown is emitted after this pass,
so a tail that returns counts the callee-saved registers the body writes.
Without it queens m64 -O2 was +24 B (copies the tail merge joined again).

DEVIATE TL2 (no number): `Preds` is rebuilt after each tail copied, so a
function with many copied tails costs one pass over its blocks each; the pin
updates the CFG in place.

## Example
The unit tests: a join of two arms holding one `mov` is copied into both
and removed; the same join holding a call is kept.

## Used by
`flow::machine_with`, directly before block placement, when the target
asks for it.
