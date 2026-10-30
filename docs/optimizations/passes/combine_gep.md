> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_gep.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_gep.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_gep.rs sha256:0b2fc1e01cc9f35d003bd6c8ee160818bfc011495b2ca285dc5baf2d10686c71 -->

# combine_gep

`visitGetElementPtrInst`: canonical forms of a `getelementptr`.

Translation of
[`InstCombinerImpl::visitGetElementPtrInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L3321)
and `emitGEPOffset`
([Local.cpp:22](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/Local.cpp#L22))
at d1106deb71cc, up to the one-index rewrites (:3513), with `visitGEPOfGEP`
in `combine_gep_merge.rs`; the rest follows in later PRs.

MIRROR: indices over a zero-size type become zero, an index is cast to the
layout's index type, a GEP of constants is a byte `ptradd`, a GEP whose
single index is a multiply or shift by a constant, or whose base is a
constant-index GEP, is a `ptradd` of its byte offset, trailing and leading
zero indices are dropped, a GEP with two non-zero indices is split in two,
and a single-index GEP over `T` is over `[sizeof T x i8]`.

DEVIATE 1: no vector or scalable GEPs: MIR has neither.

DEVIATE 2: `simplifyGEPInst` is `combine::visit`'s `simplify_instruction_in`
before this runs.

DEVIATE 3: a source type changed in place returns `Visit::Changed` (the
instruction is looked at again and finds nothing more) where LLVM sets
`MadeIRChange` and goes on.

## Not yet translated
`foldGEPOfPhi`, the single-index rewrites (:3513-3640), the
derived `inbounds`/`nuw` (:3641-3680), `foldSelectGEP` and the `srem` form.

## Example
```text
%p = getelementptr [4 x i32], ptr %b, i64 0, i64 1      %p = getelementptr i8, ptr %b, i64 4
```

## Used by
Named by `combine_compares`, `combine_gep_base`, `combine_gep_index`, `combine_gep_merge`, `combine_gep_phi`.
