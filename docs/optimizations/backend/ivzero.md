> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/ivzero.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/ivzero.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/ivzero.rs sha256:4aefc624952fd88294a3da07038c7642f3ae808710ff4c22df3d31b2666d75e8 -->

# ivzero

A loop's index ends at zero (DEVIATE LSR3).

A counted loop that tests `i != K` after its step reads `i` only as an
address, so `j = i - K` does the same: the bound disappears, the step's own
flags end the loop, and each address that used `i` takes `K` more in its
displacement. The pin's LoopStrengthReduce does this for every loop
(`ICmpZero` uses); ours reduces the innermost loops only
([`crate::lsr_pass`](../passes/lsr_pass.md)), and outer-loop LSR grew rows its register model could
not see (the inner body's other live values). Here `j` replaces `i`, so
the registers held are the same by construction.

```text
  bx = 0                          bx = -9
  L: movzx ecx, [data+bx]    =>   L: movzx ecx, [data+9+bx]
     ...                             ...
     inc bx                          inc bx
     cmp bx, 9                       jne L
     jne L
```

## Example
The loop above: `cmp bx, 9` goes, `bx` starts at -9, and `data` takes 9 more
in its displacement.

## How it works
A loop of one latch and a preheader that leaves by one jump. The latch ends
`step; sub t, step, K; jcc header` with `t` read by nobody, `step` an `inc`,
`dec`, `add` or `sub` of the loop's phi `i` by a constant, `jcc` an equality
test (`peepholeopt::condition_of`), reading only flags the step writes. `i`
starts at a constant. Every other reader of `i` is a memory operand
naming it as base or index, whose displacement, shifted by `K` (times the
scale for an index), still fits the address and the encoding it had: a
frame cell is left, and so is a displacement that would grow from one byte
to the full width. `step` is read by the test and the phi only, and `i`
by no instruction outside the loop. The phi becomes `j` starting at
`start - K`, the test is gone.

Bytes: the compare goes (2 or 3 B); the start is a `mov` of an immediate
either way, one byte more than `xor` of zero. Never larger at any level.

Complexity: O(instructions) per loop.

DEVIATE LSR3 (owner w12, exit: the pin's LSR over outer loops once its
register model sees the inner body's pressure, and the allocator
splits what the pin's does): no pin mechanism at this level; precedent
LLVM's `ICmpZero` formulae in LSR and gcc's `iv_elimination_compare`.

## Runs
A machine phase after `IvShare`, at every -O level but -O0.
