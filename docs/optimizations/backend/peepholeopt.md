> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/peepholeopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/peepholeopt.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/peepholeopt.rs sha256:f4d512f7764bef2e7bd2aee270559b922d395867f16c61de09109191de13d463 -->

# peepholeopt

A subtract whose difference nothing reads is a compare.

Translated from the first switch of `X86InstrInfo::optimizeCompareInstr`
([X86InstrInfo.cpp#L5477](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5477)),
which the pin's PeepholeOptimizer runs before allocation on every
`SUB` it can analyse as a compare.

## How it works
`EmitCmp` builds `X86ISD::SUB` for a compare, "to enable CSE between SUB and
CMP". After MachineCSE and MachineSink, a `SUB` (register, immediate or
memory second operand) with no reader of its result writes only the flags,
which is what `CMP` does: the destination goes and the opcode becomes the
compare's. Left as a `SUB`, the dead result gets a register, and a copy
to keep the operand it overwrites.

```text
 d := sub a, b      (nothing reads d)         cmp a, b
 jl                                           jl
```

A compare of a value with zero (`test x, x`, `cmp x, 0`) goes where an
earlier instruction of the block that made `x` set the flags it reads:

```text
 x := shr y, 2                      x := shr y, 2
 test x, x           =>             jne L
 jne L
```

The instruction must be one `isDefConvertible` lists (here `add`, `sub`,
`inc`, `dec`, `neg`, `and`, `or`, `xor` and `shl`/`shr`/`sar` by a count
that is not zero), no instruction between it and the compare may write the
flags, and every reader of the flags after the compare must read only what
the instruction sets as a compare of zero does: ZF always, SF unless
`NoSignFlag`, OF only where the instruction clears it (`and`, `or`, `xor`);
never CF. The flags must not live out of the block.

Not translated: a compare redundant with another compare or subtract
(`isRedundantFlagInstr`, `IsSwapped`, `ImmDelta`), `isUseDefConvertible`
(`popcnt`), `adc`/`sbb`/`shld`/`shrd`/`lzcnt`/`tzcnt` and the BMI group as
the flag source, the backward scan into a single predecessor, `MOV32r0`
motion and the NF variants (APX).

## Runs
A machine phase on SSA after MachineSink and ahead of MachineCombiner (the
pin's `addMachineSSAOptimization` order), at every level but -O0.
