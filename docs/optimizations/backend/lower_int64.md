> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/lower_int64.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/lower_int64.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/lower_int64.rs sha256:7f4d51822a7176fff0a478d8e63bd2a63faa2bf986d4af8b94cba316a494c563 -->

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

## Used by
`isel/wide` (64-bit arithmetic helpers) and `isel/unwind`.
