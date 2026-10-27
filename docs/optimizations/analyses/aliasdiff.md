> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/aliasdiff.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/aliasdiff.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/aliasdiff.rs sha256:53dcf8c74f408bec844535f08a67329b009017a5e9c50fdf7abacdbf5c0731c5 -->

# aliasdiff

The alias stack's answers, recorded and compared: the gate for any change
to what the stack is built from.

## How it works
`alias_pairs` (bin) prints, per function of a module dump, one row per
memory access `i` holding the verdict of the stack for the pair `(i, j)`
for every later access `j` (`N` no alias, `M` may, `P` partial, `U` must),
and one row per call holding, for every access, what the call may do to its
location (`0` nothing, `R` read, `W` write, `B` both). A row is keyed by
the function and the position of the access or call in the function's
walk, which is the same for two builds fed one dump.

[`compare`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/scev/compare.rs) reads two such recordings. A pair that was `N` and is not, or
a call that touched a location and no longer does, is *lost* (precision;
counted). A pair that was not `N` and is, or a call that read or wrote a
location and now does not, is *gained* (a new proof): it fails, since a
change that only moves the analyses' inputs may remove answers, never
invent them. A forged segment pointer answered `N` by the new stack is the
`b$seg` case this guards.

## Example
`a_pair_that_becomes_no_alias_is_gained` in `aliasdiff_tests.rs`.

## See also
the alias-stack plan (stage S0.5).

## Used by
Named by `alias_pairs`.
