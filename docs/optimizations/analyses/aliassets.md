> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/aliassets.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/aliassets.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/aliassets.rs sha256:3caec1fc2c64b0b420f6de7750f0f8033b9c6cb80a6c15aa45824593fedeb3ef -->

# aliassets

`AliasSetTracker` (LLVM `AliasSetTracker.cpp`, `d1106deb71cc`): the
locations an analysis asks of, grouped into sets that may alias, with
whether each set's locations all must alias. Over the locations of
`mssa::alias::AliasQuery`.

DEVIATE: the reference counts and the intrusive list are not kept: a set
that forwards is skipped by every iteration, and one that does not is
referenced by the pointer map, so no set is ever removed. Unknown
instructions (`addUnknown`) are not tracked: LICM adds loads and stores
only.

## Example
Two stores through pointers that may alias share a set; through pointers
that cannot alias they are two sets:

```text
store i32 1, ptr %a
store i32 2, ptr %b   ; one set if %a and %b may alias, two if they do not
```
