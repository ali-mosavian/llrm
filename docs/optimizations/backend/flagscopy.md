> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/flagscopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/flagscopy.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/flagscopy.rs sha256:79c32d9e66ae9efe7f7ef3df058c547d3c963335a49cdec9387f9619489c0eda -->

# flagscopy

Flags kept across an instruction that clobbers them: `X86FlagsCopyLowering`.
Translated from
[X86FlagsCopyLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp),
function by function, in the pin's order.

## How it works
A copy of the flags is the pin's two `COPY`s as two pseudo instructions:
`v := flags_copy` (`%1 = COPY $eflags`) and `flags_restore v`
(`$eflags = COPY %1`). For each restore, in reverse post-order, the
conditions the code after it reads are saved as bytes by `setcc` at the
copy (`promote_cond_to_reg`, [L744]) and each reader is rewritten:
a `setcc` takes the saved byte (`rewriteSetCC`),
`adc`/`sbb`/`rcl`/`rcr` rebuild the carry with `add byte, 255`
(`rewriteArithmetic`), any other reader (`jcc`, `cmov`) is preceded
by `test byte, byte` and reads `ne`, or `e` of the opposite condition's byte
(`rewriteMI`, `getCondOrInverseInReg`,
`insertTest`). A reader after the first jump of a block needs the
block split (`splitBlock`).

Pin functions in file order: `isArithmeticOp` ([L144]) is `reads::Arith`;
`splitBlock` ([L149]) is `split_block`; `getClobberType` ([L251]) only
serves the NF variants, which the target lacks (`hasNF` is APX: N/A);
`runOnMachineFunction` ([L263]) is `lowered` and `lower_copy`;
`collectCondsInRegs` ([L721]), `promoteCondToReg` ([L744]),
`getCondOrInverseInReg` ([L761]), `insertTest` ([L775]), `rewriteSetCC`
([L785]), `rewriteArithmetic` ([L824]), `getImplicitCondFromMI` ([L855],
the x87 `fcmov`s: LIR has none before allocation, N/A) and `rewriteMI`
([L899]) keep their names in snake case.

[L144]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L144
[L149]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L149
[L744]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L744
[L761]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L761
[L775]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L775
[L785]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L785
[L824]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L824
[L899]:
https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L899

Cross-block lowering follows the pin: the copy may be in another block than
its restore, the test position is hoisted up the dominator tree while the
flags live in and nothing clobbers them (`HasEFLAGSClobber`,
`HasEFLAGSClobberPath`), the flags live into successors are rewritten
where they are read (the copy's block must dominate them; else the pin's
fatal error, here an `Err`), and a second `jcc` on the flags goes to a block
split off (`splitBlock`) with its edges, probabilities and phis.
The NF variants (`hasNF` is APX) and the x87 `fcmov` forms have no
counterpart: N/A.

DEVIATE R2 (no number yet): no isel producer of `flags_copy`/`flags_restore`
exists, so tests hand-build LIR; owner w-5, review 2026-11-19, exit an isel
producer.

## Example
```lir
v1 := flags_copy
v2 := add v3, 0x1
flags_restore v1
jl
```
becomes
```lir
v4 := setl
v2 := add v3, 0x1
test v4, v4
jne
```

## Runs
After `FixupSetCC`, before the float phases. Every level (the pin's pass is
not gated), so it is not in `UNOPTIMIZED`.
