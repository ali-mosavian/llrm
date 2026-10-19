> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/fpcopyprop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/fpcopyprop.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/fpcopyprop.rs sha256:1dba4611686d14b736ac04ce2a48a8a78ed57d0f387ce27b9041cb42e401b55a -->

# fpcopyprop

Copy propagation over the x87 registers FP0..FP6: LLVM's
`MachineCopyPropagation` for the RFP class.

## How it works
Registers are chosen and the stackifier has not run. Each block is walked
forward with the copies `FPd := FPs` still valid (neither side written
since): a read of `FPd` reads `FPs`, and a copy that repeats a valid one is
dropped. A backward walk then drops a copy whose destination nothing reads
before it is written or the block ends, and that is not live out.

```text
  fp3 := fp1                  fp3 := fp1      <- dead when fp3 is not
  fstp [m], fp3        =>     fstp [m], fp1      read again
```

A removed copy becomes an anchor (`lir::anchor`), as in `machinedce`.

DEVIATE 1: `machinecp` tracks the byte lanes of the general registers from
the decoded effects of an instruction. An x87 operation here has no
encoding yet, so its reads and writes are its operands; this is a second
tracker of seven units. Exit: the stackifier follows registers and the
tracker is `machinecp`'s with the RFP class as more units.

DEVIATE 2: a copy in a group (a phi's parallel copy) is neither rewritten
nor removed; its writes end the copies they touch.

## Runs
At the start of `FloatAlloc`, under `LLRM_GREEDY_RFP`.

## Example
The listing above: the store reads `fp1`, the copy is dead and gone.

## Needs
[`crate::backend::fpregs`](fpregs.md) for the registers live out of a block.

## See also
LLVM `MachineCopyPropagation`
(https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp)
