> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/latecleanup.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/latecleanup.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/latecleanup.rs sha256:865b4104646688ce4d6f434865f1aa6bd1ee734f5dd6467c02c61ec098846f12 -->

# latecleanup

Removes an immediate load to a register that an identical, earlier load in
every path to it already left there: LLVM's `MachineLateInstrsCleanup`
([MachineLateInstrsCleanup.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLateInstrsCleanup.cpp)).

## How it works
Blocks are taken in reverse post-order. Each block starts with the loads
every predecessor ends with identically (`RegDefs` of the first predecessor
that all the others hold the same entry of; a predecessor not yet walked, a
back edge, holds none). An instruction that writes a register's lanes drops
the entries on those lanes, and one with an unknown effect drops them all. A
candidate is a pure `mov reg, imm` writing no flag: one identical to the
held entry of its register is replaced by an anchor, else it becomes the
entry.

```text
  b0: mov ax, 5            b0: mov ax, 5
      jcc b2                   jcc b2
  b1: mov ax, 5   ==>      b1: (anchor)
      ...                      ...
```

## Example
The unit tests `a_load_the_register_already_holds_goes` and
`a_load_after_another_path_changed_the_register_stays`.

## Runs
After `prologue`, before `peephole`: where rematerialization and the
allocator's splitting have left the same constant loaded again.

## Limits
DEVIATE LC1: the pin also takes a load-address instruction using the frame
register (frame-index anchor points made by frame elimination); here the
frame is addressed by `masm`, so only immediates are candidates. There are
no kill flags in LIR, so `clearKillsForDef` has nothing to clear.
