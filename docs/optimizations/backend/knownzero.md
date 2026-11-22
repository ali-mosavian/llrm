> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/knownzero.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/knownzero.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/knownzero.rs sha256:59cebff7573ce097ef173b61bfce41fa2ee8f0f021a39f51c16700322f4a700e -->

# knownzero

A move into a register that already holds the same zero: GCC's
`postreload-cse` (`reload_cse_regs`, `reload_cse_simplify_set`) for the one
constant every function makes the most.

## How it works
A forward dataflow over the allocated body keeps the byte lanes known to be
zero before each instruction. `xor r, r`, `sub r, r` and `mov r, 0` set the
lanes of `r`; `mov d, s` sets the lanes of `d` that `s` holds zero in; any
other write clears what it writes, and an instruction of unknown effect
clears all. A block starts with the lanes every edge into it leaves zero
(the entry block with none). Then a `mov d, s` whose `d` and `s` are both
zero, and a `mov d, 0` whose `d` is, change nothing and are dropped. `xor r,
r` stays: it writes the flags.

```text
  xor edx, edx                 xor edx, edx
  test ax, ax                  test ax, ax
  ...                          ...
  xor eax, eax                 xor eax, eax
  rep stosb                    rep stosb
  mov dx, ax      =>           (dropped: dx and ax are zero)
```

`machinecp` (LLVM's `MachineCopyPropagation`) sees the copies of one block
and no values, so it cannot tell that `dx` already held the zero `ax` holds.
LLVM has no counterpart: its `MachineCSE` runs before the phis become copies
and does not share a constant a phi reads (heuristic 3).

## Example
m16 `sieve` -Os: the count (`dx`) and the sum (`eax`) both start at zero,
the count's zero made at entry, the sum's after the early exit; `mov dx, ax`
is the second zero into `dx`: 112 B, one over Open Watcom's 111.

## Runs
After the last `machinecp`, on x86 bodies with no phis.

## See also
[`crate::backend::machinecp`](machinecp.md); GCC
[`postreload.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/postreload.cc).
