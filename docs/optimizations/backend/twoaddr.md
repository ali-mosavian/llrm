> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/twoaddr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/twoaddr.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/twoaddr.rs sha256:12f2c5a50306c0fbac4539b1e8f498fd87e7671e95cc1e87a91494a957982d5c -->

# twoaddr

Rewrites three-address arithmetic into x86's two-address form before
register allocation.

## How it works
An instruction that writes the register of its first source (`Binary`,
`Unary`, `Funnel`, and a one-destination `Multiply`) is split into a copy
and the instruction on the copy:

```text
 c = add a, b        ==>      c = mov a
                              c = add c, b
```

Three things happen first so the copy is less often needed.

- Update in place: `c = add a, b` followed by the copy `a = mov c`, with `a`
  dead between, and `c` defined once and read only by that copy, becomes `a
  = add a, b`. No interval grows and the copy the coalescer might refuse is
  gone.
- Commuting: for `add`, `and`, `or`, `xor` and `imul` the sources are
  swapped when that lets the destination reuse a source that dies. A swap is
  chosen when the second source is the destination already, when only the
  second source dies, or when neither lives on and the second is less
  blocked (by interference with the destination's copy neighbours) and
  closer by copies. Interference is computed only among the values a swap
  asks about.
- A memory destination whose cell is not the first source (`[m] = add a, b`)
  is computed in a fresh register: load, operate, store.

A source that is not a value (an immediate) is copied the same way. The
inserted copies own no bytes of the input.

## Example
The preheader of a loop, before and after. `#25` reads `y6.1`, which
`#26` reads later, so it takes a copy (`#35`). `#26` is the last use of
`y6.1`, so its copy is a self-move (`#36`) left for the coalescer:

```lir
═══ _mark@3 · FloatAlloc · m16 ═════════════════════════════════════════════════
fn _mark@3(t1@ax, t2@dx, y6.1@cx)
     ; ...
B3  ← B0  → B2                                                    ×1.0
    #25  lsr.iv     = shl.i16     y6.1, 1
    #26  t6         = sub.i16     y6.1, t4
    #30  lsr.iv1    = mov.i16     t6                                    ; copy
    #27               jmp         B2

═══ _mark@3 · TwoAddress · m16 ═════════════════════════════════════════════════
fn _mark@3(t1@ax, t2@dx, y6.1@cx)
     ; ...
B3  ← B0  → B2                                                    ×1.0
    #35  lsr.iv     = mov.i16     y6.1
    #25  lsr.iv     = shl.i16     lsr.iv, 1
    #36  y6.1       = mov.i16     y6.1
    #26  y6.1       = sub.i16     y6.1, t4
    #30  lsr.iv1    = mov.i16     y6.1                                  ; copy
    #27               jmp         B2
```

From `ISEL_DUMP=1` of `mark` in tests/fixtures/c/farloadloop.c with
`-m16 -O2`, stages `FloatAlloc` and `TwoAddress`.

## Runs
A machine phase after `floatalloc` and before `coalesce`
(`flow::machine_with`), at every level. The coalescer then removes the
copies the allocator can share a register across.

## See also
LLVM's
[`TwoAddressInstructionPass`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp).
