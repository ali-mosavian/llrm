> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ivutil.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ivutil.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ivutil.rs sha256:54ccc5e1e5a66bc7902984e5e31c2ab42c5d308ee4412701a90e2f3ce50f1414 -->

# ivutil

What the counter-rewriting passes ([`crate::maskscale`](maskscale.md), [`crate::ivscale`](ivscale.md))
read of the IR: an integer constant, an operand's defining instruction, a
value wrapped to a width, and a constant of a type.

## Example
`int(context, i16, 1 << 16, 16)` is the 16-bit constant 0.

## Used by
Named by `ivscale`, `maskscale`.
