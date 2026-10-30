> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/registerinfo.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/registerinfo.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/registerinfo.rs sha256:02eafa3a5eb7d0ee4dd8184fed16eea7e70889e6b3e44d5174587d124094d379 -->

# registerinfo

The backend's view of a target's register file: re-exports of the types a
target's `registers.regs` description generates.

## How it works
A target lists its registers in `registers.regs`: name, bit width, root
register, lane (bit offset in the root), classes (`gpr`, `byte`, `base`,
`index`, `frame`, `stack`, ...) and debug-format numbers, plus the far-load
mnemonic of each segment register and the order scratch registers are
borrowed in. The target crate generates an `Info` table from it (m16's is
`llrm_x86_m16::REGISTER_INFO`) and a `Target` hands it out.

`Info` answers every query of a register: its width (`bytes`), its `root`,
the `view` of a root at a width, `lanes`, whether it is the frame or stack
register, which segment register means data, stack, code or far, which
mnemonic loads a selector into it, whether two registers overlap. `Regs`
wraps a `&'static Info` so a pass carries the file it was given instead of
reading a global. A `RegId` is `iced_x86::Register` for now.

This file re-exports `Entry`, `FRAME`, `Info`, `RegId`, `Regs`, `STACK` and
`class` from `llrm_lir::registers` under the path the backend uses, and,
under test, `test_regs()`, m16's file for tests that compile for no target.
Its tests check the description against the hand tables in
`llrm_x86::registers`.

## Example
None: it holds re-exports, so there is no before and after. Read
`registers.regs` of a target for the table `Info` is generated from.

## Used by
[`crate::backend::allocate`](allocate.md), `crate::coalescer::pass`,
[`crate::backend::liveness`](liveness.md), [`crate::backend::loopslots`](loopslots.md),
[`crate::backend::masm`](masm.md), [`crate::backend::objbuild`](objbuild.md) and the peephole.
