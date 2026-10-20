> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/laa_testing.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/laa_testing.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/laa_testing.rs sha256:540a36106c0aa4971f93b46e6e4a050cc91628242b7f84cbd63f7f223a35abf9 -->

# laa_testing

How `opt` writes the values `print<access-info>` lists, for the tests that
compare with it: the instruction lines of the function as MIR prints them
(the same text, in order, for the same names), and an argument as `ptr %x`.
