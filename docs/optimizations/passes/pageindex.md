> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/pageindex.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/pageindex.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/pageindex.rs sha256:6c3021d2cde6f391373f3155c27516fea63f04ee38b263092fc32cfaf0363075 -->

# pageindex

Pointers that step through memory a byte at a time, as a base and a byte
index, before instruction selection.

## How it works
A loop with a pointer `p` that starts at `s` and steps by `c` bytes is
`p = s + c * trips`. On a target whose indexed load adds an index of `n`
bits to a base (`Machine::index_register_bits`), the sum is cheaper kept in
two parts: an index `i` that steps by `c`, and a base that moves by `2^n`
where the index wraps. The loop's pointers share the one index.

The pass rewrites the loops of one latch and one way in whose pointers are
used only as the address of a byte load or store (or of a constant offset
from it). Pointers it cannot rewrite keep their own steps. A loop rotated to
load the next byte before it tests steps in the latch at that load, and the
load reads the next page's base through a join block after it.

DEVIATE (6502-6): it is not LLVM's. `MOSIndexIV` (llvm-mos) rewrites a
pointer to a base and an 8-bit index only where `ScalarEvolution` proves the
index stays under 256; this one handles the wrap with a block run once every
`256 / c` trips. Reason: the string and memory routines of the C library
walk a pointer of unknown extent. Exit: `MOSIndexIV` translated for the case
`ScalarEvolution` proves under `2^n`; the wrap block stays as the deviation.

## Example
`strcpy`'s loop, in IR:

```text
loop: %d1 = phi [%d, pre], [%d2, loop]    %s1 = phi [%s, pre], [%s2, loop]
      %c = load i8, ptr %s1 ; store i8 %c, ptr %d1
      %s2 = gep %s1, 1 ; %d2 = gep %d1, 1 ; br %c != 0, loop, exit
```

becomes

```text
loop: %bd = phi [%d, pre], [%bd, step], [%bd2, bump]    (same for %bs)
      %i = phi i8 [0, pre], [%i1, step], [%i1, bump]
      %ps = gep %bs, zext %i ; %pd = gep %bd, zext %i
      %c = load i8, ptr %ps ; store i8 %c, ptr %pd ; br %c != 0, step, exit
step: %i1 = add i8 %i, 1 ; br %i1 == 0, bump, loop
bump: %bd2 = gep %bd, 256 ; %bs2 = gep %bs, 256 ; br loop
```

## Runs
In a target's driver, after the mid-end, for a target whose
`Machine::index_register_bits` is set: the width of the index its indexed
addressing adds to a base (8 for `lda (base),y`).
