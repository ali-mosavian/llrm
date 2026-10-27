> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/sharedstores.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/sharedstores.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/sharedstores.rs sha256:daa52e2eab231f55f150e38b178ae8244c109ad716bb700fa8226493c2f21ab0 -->

# sharedstores

At -Os, a run of stores of one literal becomes the literal loaded once into
a dead register, then register stores.

## How it works
`mov dword ptr [bp-8],0` is 8 bytes, `mov [bp-8],eax` is 4 and the one `xor
eax,eax` is 3, so two stores or more win.

```text
 before                          after
 mov dword ptr [bp-8],0          xor eax,eax            ; 3 bytes
 mov dword ptr [bp-4],0          mov [bp-8],eax         ; 4
 mov dword ptr [bp-12],0         mov [bp-4],eax         ; 4
                                 mov [bp-12],eax        ; 4
 24 bytes                        15 bytes
```

A run is consecutive plain `mov` stores of an immediate in one block, with
no relocation on the literal. All stores of a run hold the same value; zero
may mix widths, any other literal needs the same width. The register is
taken from the allocatable ones, skipping a register the epilogue would have
to save and the body does not already name (it would be pushed and popped
for the run). It must be dead after the last store and not read by any store
of the run (an address register). It is as wide as the widest store;
narrower stores use the matching sub-register.

The load is `xor r,r` when the literal is zero and the flags are dead at the
run's start, else `mov r,imm`. The change is made only if the encoded sizes,
priced by the selector, are strictly smaller than the run's.

## Example
Four stores of zero through `BX`, with `AX` dead after the argument is
copied. Before, the input of `Peephole`:

```lir
=== ParallelCopy
  _f@3: 7 instructions
    block 0x0000
      0x0000     ; inserted; delivers v2@AX
      0x0000  BX := mov AX  ; inserted
      0x0001  [None v1@BX] := mov 0x0  ; inserted
      0x0003  [None v1@BX] := mov 0x0  ; inserted
      0x0005  [None v1@BX] := mov 0x0  ; inserted
      0x0007  [None v1@BX] := mov 0x0  ; inserted
```

After:

```lir
=== Peephole
  _f@3: 8 instructions
    block 0x0000
      0x0000  BX := mov AX  ; inserted
      0x0001  AX := xor AX, AX  ; inserted
      0x0001  [None v1@BX] := mov AX  ; inserted
      0x0003  [None v1@BX] := mov AX  ; inserted
      0x0005  [None v1@BX] := mov AX  ; inserted
      0x0007  [None v1@BX] := mov AX  ; inserted
```

One `xor` and four register stores replace four immediate stores. The dump
omits the stores' displacements.

From `ISEL_DUMP=1` of `void f(struct s *p)` storing 0 to four `int` fields
at -Os -m16.

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), once per body, after
`zeroed_loads` and before `machinedce`. It returns the body unchanged unless
the CPU profile is sized for bytes (`cpu.size`, -Os): the load costs a
clock.
