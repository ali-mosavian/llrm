> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fpcopyprop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fpcopyprop.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fpcopyprop.rs sha256:e4e538463a85685f89048fa7286463ec98d704f4ce9677744a09a40e96ba3f1d -->

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

A float rounded through a cell and stored on, `fstp [t]; fld [t]; fstp [h]`
with both cells the same width, rounds once into `h` as the first store
does: the first store is dropped and `h` takes its source, where nothing
reads `t` before a store rewrites it. pow_stack_depth rounded each of its
six doubles twice (m16 -Os 1644 B against main's 1462).

Stored back into `t`, the reload and second store go and the first stays.

```text
  [t] := fstp fp0             [h] := fstp fp0
  fp1 := fld [t]       =>
  [h] := fstp fp1
```

DEVIATE 1: `machinecp` tracks the byte lanes of the general registers from
the decoded effects of an instruction. An x87 operation here has no
encoding yet, so its reads and writes are its operands; this is a second
tracker of seven units. Exit: the stackifier follows registers and the
tracker is `machinecp`'s with the RFP class as more units.

DEVIATE 2: a copy in a group (a phi's parallel copy) is neither rewritten
nor removed; its writes end the copies they touch.

## Runs
At the start of `FloatAlloc`, under RFP.

## Example
The listing above: the store reads `fp1`, the copy is dead and gone.

## Needs
[`crate::backend::fpregs`](fpregs.md) for the registers live out of a block.

## See also
LLVM `MachineCopyPropagation`
(https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp)
