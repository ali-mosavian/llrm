> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/congruentiv.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/congruentiv.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/congruentiv.rs sha256:e51c5b2ec36465a130c09a21a4527508a49026702f92a4b60637316aee93cf14 -->

# congruentiv

Phis of a loop header that ScalarEvolution proves equal are one phi.
LLVM's `SCEVExpander::replaceCongruentIVs`, as `IndVarSimplify::run` calls
it after the exit values.

## How it works
For each loop in LoopSimplify form, innermost first, the header phis are
taken from the widest integer to the narrowest. A phi that simplifies to a
value, or whose SCEV is a constant, is replaced by it. A phi whose SCEV an
earlier phi has is replaced by that one (a truncation of it where it is
narrower), and its increment by the other's. A wide counter that truncates
free is also the representative of its truncation, so a narrow counter
that is the `trunc` of a wide one reads the wide one.

## Example
`%i = phi i32 [0,..],[%i1,..]` and `%j = phi i16 [0,..],[%j1,..]` with
`%i1 = add %i, 1`, `%j1 = add %j, 1`: the readers of `%j` read `trunc i32
%i to i16`.

## Used by
[`crate::indvars`](indvars.md), which leaves the phis it replaced to `dead`.

## See also
`docs/architecture/scev-port.md`.
