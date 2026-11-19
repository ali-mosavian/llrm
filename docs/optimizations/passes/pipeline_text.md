> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/pipeline_text.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/pipeline_text.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/pipeline_text.rs sha256:b48a88a6f88075140b15495f63199d84e282d9f2cabac358f2899d6d6a5cdc07 -->

# pipeline_text

The pass pipeline text of `opt -passes=`: `a,b(c,d),e<x;y>`. `parse` reads
the syntax (`PassBuilder::parsePassPipeline`); `build` adds the passes this
crate has to a manager and names the first it does not have.

## Example
`loop(indvars,loop-reduce),dce` parses to a nested `loop` adaptor holding
two passes, then `dce`:

```text
Nested loop [ Pass indvars, Pass loop-reduce ], Pass dce
```

## Used by
Named by `main`.
