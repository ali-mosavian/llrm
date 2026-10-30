> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpswitch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpswitch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpswitch.rs sha256:cfe0b35721ac401dbc7e75a186c61e1e548c8fc5cdef4ed011cffeaa6982d558 -->

# cgpswitch

CodeGenPrepare's `optimizeSwitchType`: a switch on an integer narrower than
the target's preferred width is a switch on the extended condition
([CodeGenPrepare.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp)).

## How it works
`switch i16 %x` with cases 1, 2, 3 becomes `%w = zext i16 %x to i32` and
`switch i32 %w` with cases 1, 2, 3 as `i32`, so the comparisons or the jump
table's range check are made on a full register and not on a 16-bit one
beside an extension of the index. The extension is `zext`, `sext` when the
condition is a `signext` argument; a `zeroext` argument is `zext`.

MIRROR: the whole function.

DEVIATE SW1: the target's preferred width is
`Lowering::preferred_switch_condition_bits`. Exit: none.

## Example
`test_a_switch_on_an_i16_is_a_switch_on_its_zext_to_i32` in
`cgpswitch_tests.rs`.

## Runs
In `cgpaddr`'s run.
