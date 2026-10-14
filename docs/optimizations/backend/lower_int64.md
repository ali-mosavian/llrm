> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/lower_int64.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/lower_int64.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/lower_int64.rs sha256:9f0cfb8a0920b9fef3ca6b26e1fe7e563c1a4b2a3db9e3650c581fae5ef0d364 -->

# lower_int64

The contract of an inline routine the compiler places where an operation
has no instruction (`llrm_x86::helpers`): what it reads and what it changes.

## How it works
`_helper` makes the `crate::abi::runtime::Contract` of such a routine: it
returns, pops nothing, reads and writes no memory, and clobbers the
registers it is given. Its operands and results are in registers it
states, and the clobber list describes the inline bytes exactly (it is not
a separately called routine). `runtime_registers` maps register views
(`AX`, `BL`, ...) to the runtime registers they belong to, and adds the
flags where the routine changes them.

## Example
No honest example: this builds a routine's contract and rewrites no LIR.
Read `isel/wide.rs` for the instructions that name it.

## Used by
`isel/wide` (64-bit arithmetic helpers) and `isel/unwind`.
