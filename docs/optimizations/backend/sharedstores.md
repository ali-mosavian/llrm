> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/sharedstores.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/sharedstores.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/sharedstores.rs sha256:7ec4c076ee651e9bd60cad6af3526e599298e236546aefeca80543c7ed2d807e -->

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

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), once per body, after
`zeroed_loads` and before `machinecse`. It returns the body unchanged unless
the CPU profile is sized for bytes (`cpu.size`, -Os): the load costs a
clock.
