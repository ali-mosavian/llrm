> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/saverestore.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/saverestore.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/saverestore.rs sha256:022fb71cee4883098eee75b7d65ed4721f69bd13c3f3faa2532fd21534c67e54 -->

# saverestore

Where a procedure's frame and kept registers are set up and taken back: one
Save block and one Restore block, from `ShrinkWrapImpl` in
llvm/lib/CodeGen/ShrinkWrap.cpp at d1106deb71cc.

The pass reads the dominator and post-dominator trees, the cycles and the
block frequencies of the body, and what each block does with the frame
(`Frame`); it changes nothing. `run` answers with the `Placement` the
procedure's prologue and epilogue go to, or none where the entry is as good.

DEVIATE 1: no EH pads, funclets or `INLINEASM_BR` targets: LIR has none, so
their arms are absent.
DEVIATE 2: `postShrinkWrapping` does not make the split block, it names it
(`Placement::split`); the consumer puts the epilogue on those edges.
LLVM then recalculates the trees only for its asserts, which are not kept.
DEVIATE 3: `RegScavenger`, `getCurrentCSRs` and the frame-index arms of
`useOrDefCSROrFI` are the caller's `Frame::touches`: what is a kept
register, a frame cell or the stack pointer is the target's to say.
