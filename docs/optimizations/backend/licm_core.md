> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/licm_core.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/licm_core.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/licm_core.rs sha256:22c14d218ac159c9a068fba416e5ba5aa7bc3f6e9c6d3ea9f2e01254670a8a40 -->

# licm_core

The target-neutral half of early `MachineLICM`: which loops have a
preheader, which values a loop reads from outside, which copies a loop
phi takes, and the walk that moves what the target's policy calls
hoistable to the end of the preheader. Split out of `machinelicm.rs`
(`HoistOutOfLoop` L827, `Hoist` L1615 of
[MachineLICM.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp))
so the x86 LIR and the arm64 machine function run one pass.

## How it works
The caller states its function as `Block`s of its own instruction type.
[`hoist`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/licm/hoist.rs) takes the loops inner first. A loop with one block outside it
that enters the header, and that block leaving only for the header, has a
preheader. Each such loop is walked in layout order until nothing moves; the
`Policy` says for each instruction whether it moves, given `Facts` of
the loop. A moved instruction goes before the preheader's jump.

## Example
```text
  pre:  jmp loop                      pre:  v9 := adrp @g ; jmp loop
  loop: v9 := adrp @g            =>   loop: use v9
        use v9
```

## See also
`machinelicm` (the x86 policy), the arm64 policy in `llrm-arm64`.
