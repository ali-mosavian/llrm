> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/lower_int64.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/lower_int64.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/lower_int64.rs sha256:ac3cd7b21978234d04c0f7349a45bd2469fddd39f0da39304252d135fed345b8 -->

# lower_int64

The contract of an inline routine the compiler places where an operation
has no instruction (`llrm_x86::helpers`): what it reads and what it changes.

## How it works
`_helper` makes the [`crate::abi::runtime::Contract`](https://github.com/ali-mosavian/llrm/blob/main/crates/driver/llrm-cc/tests/abi.rs) of such a routine: it
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
