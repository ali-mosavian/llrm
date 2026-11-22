> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/postralicm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/postralicm.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/postralicm.rs sha256:3d0cb3d972f7ebfb98db231d677eb15ec3e9cfd46ff9166a50f15f2798274f64 -->

# postralicm

Moves a constant a loop makes on every trip to the loop's entry.

## How it works
The pin's post-allocation MachineLICM (`HoistRegionPostRA`, `ProcessMI`,
`HoistPostRA`). After allocation an instruction reads no value the loop
changes only if it reads no register at all, so what it moves is an
instruction that makes a constant (`mov r, imm`, the address of a symbol,
a zeroing `xorps x, x`) into a register that the loop defines nowhere else
and that no block of the loop is entered with a live value in. The
instruction goes to the end of the loop's one entry block, before its jump.

The early MachineLICM leaves the same constant in the loop when a phi in
an exit block takes it (`HasLoopPHIUse`, which rejects every exit block):
`for (...) if (c) return 0; return 1;` makes the `0` in the loop and
returns it in `eax` from the exit. Allocation then names that phi's value
`eax` alone, and this pass hoists the `xor eax, eax`.

Not ported: `getOrCreatePreheader` splitting an edge (a loop with several
entry blocks, or an entry that branches, is left as it is), and the stack
reloads, which `loopslots` moves.

## Example
`safe` of queens -O1, the zero returned from inside the loop:

```text
  loop:  mov r8d, [rdi]                 pre:   xor eax, eax
         xor eax, eax            =>     loop:  mov r8d, [rdi]
         sub r8d, edx                          sub r8d, edx
         je  exit                              je  exit
```

## Runs
After `loopslots`, before `schedule`, at every -O level but -O0.
