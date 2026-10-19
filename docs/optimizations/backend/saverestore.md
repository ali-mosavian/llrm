> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/saverestore.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/saverestore.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/saverestore.rs sha256:a86d5632d322316884d8ffe20d1bd9da5921c232b58a3d91a4ef9f3b2d411ff5 -->

# saverestore

Where a procedure's frame and kept registers are set up and taken back: one
Save block and one Restore block, from `ShrinkWrapImpl` in
llvm/lib/CodeGen/ShrinkWrap.cpp at d1106deb71cc.

The pass reads the dominator and post-dominator trees, the cycles and the
block frequencies of the body, and what each block does with the frame
(`Frame`); it changes nothing. `run` answers with the `Placement` the
procedure's prologue and epilogue go to, or none where the entry is as good.

## Example
A procedure that returns at once when its argument is 0 and otherwise calls
itself (`hanoi`). Only block 1 needs the frame, and it dominates and is
post-dominated by itself, so the procedure is set up and taken back there:

```text
  0: test; je 2          save: 1
  1: call hanoi          restore: 1
  2: ret                 (the entry and the return pay nothing)
```

This is `run` over that graph with block 1 as the only block that touches
the frame (`test_a_call_on_one_path_is_wrapped_there_and_not_at_the_entry`).

DEVIATE 1: no EH pads, funclets or `INLINEASM_BR` targets: LIR has none, so
their arms are absent.
DEVIATE 2: `postShrinkWrapping` does not make the split block, it names it
(`Placement::split`); the consumer puts the epilogue on those edges.
LLVM then recalculates the trees only for its asserts, which are not kept.
DEVIATE 3: `RegScavenger`, `getCurrentCSRs` and the frame-index arms of
`useOrDefCSROrFI` are the caller's `Frame::touches`: what is a kept
register, a frame cell or the stack pointer is the target's to say.
