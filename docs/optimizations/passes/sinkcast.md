> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/sinkcast.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/sinkcast.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/sinkcast.rs sha256:e740a5cc0b696dbdbb2ab46403fc950aeeaf7802e5466c1ff44bcf76bb1350ba -->

# sinkcast

CodeGenPrepare's `SinkCast` and `OptimizeNoopCopyExpression`: a cast read in
other blocks is made again in each, where isel takes it as a register
rename
([CodeGenPrepare.cpp#L1432](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1432)).

## How it works
Isel works a block at a time: a cast whose result crosses a block keeps the
narrow and the wide value both live. Two kinds of cast are made again in the
blocks that read them (one copy per block; a phi's read is in the block it
comes from), and the original goes once nothing reads it:
- a `zext` or `sext` to a type no register holds (`i64` here), which splits
  into two halves anyway (`TypeExpandInteger`);
- a cast that changes nothing for the registers: both types are integers (a
  pointer is one) and, once an integer narrower than a register is promoted
  to the register, they are the same type (`OptimizeNoopCopyExpression`).

```text
 b1: %p = ptrtoint ptr %a to i16        b1: ...
     br label %b2                       b2: %p = ptrtoint ptr %a to i16
 b2: store i16 %p, ptr %q                   store i16 %p, ptr %q
```

MIRROR: `SinkCast`, the two entry conditions, the promotion of a narrow
integer to the register that holds it. The pin leaves the address-space
cast alone (a free one is a target's: x86 has none).

DEVIATE SD220 (real mode): a segment made a far pointer (`addrspacecast`
from the segment space to the far or huge one) is sunk too. Far pointers
are ours, the pin has no such cast: its offset is the constant 0, and
left in one block the halves cross every block edge in registers
(`xor ebx, ebx` and a base register per store, deedlines COPPER +398 B).
Owner w12. Exit: a `isFreeAddrSpaceCast` hook on the target. Review
2026-12-10.

## Example
`test_a_pointer_cast_read_in_another_block_is_made_there` in
`sinkcast_tests.rs` is the sketch above.

## Runs
In `cgpaddr`'s run, before `optimizeExt`.
