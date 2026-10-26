> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/laa_testing.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/laa_testing.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/laa_testing.rs sha256:9aa4151af1f074493076fc4bbb2c4a1ed82ee4c96e97bcea980b79f45f451e2c -->

# laa_testing

How `opt` writes the values `print<access-info>` lists, for the tests that
compare with it: the instruction lines of the function as MIR prints them
(the same text, in order, for the same names), and an argument as `ptr %x`.

## Example
`%0 = load i16, ptr %p` as the function prints it is `%0 = load i16, ptr %p`
here, and an argument `%p` is `ptr %p`, so a test can compare the pin's
`print<access-info>` line with the pass's.
