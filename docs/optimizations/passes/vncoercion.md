> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/vncoercion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/vncoercion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/vncoercion.rs sha256:c50d4c4bde46d11334f25286bd7d88ab0c9cf1b0ccac1ef0173147f3af3e291e -->

# vncoercion

A value stored or loaded in one type, made the value a load of another type
at a must-aliased address reads: GVN's `VNCoercion`, a translation of
[`VNCoercion.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/VNCoercion.cpp)
at d1106deb71cc. The `analyze_*` functions say whether, and at which byte
of the write, a load is contained in it; `get_*` and `coerce_*` build the
value with an `Inserter`, which is `IRBuilder` at an insertion point and
folds an operation of constants as it does.

## How it works
A byte offset or `-1` in LLVM is `Option<u64>` here: none is `-1`.

DEVIATE 6, the types MIR does not have: no scalable vector, no target
extension type, no non-integral pointer (`Type` has none of them, and the
datalayout string has no `ni`). The arms of
`canCoerceMustAliasedValueToLoad`, `getStoreValueForLoadHelper` and
`analyzeLoadFromClobberingMemInst` for them are not written: no `Type` value
reaches them.

Owed by fold-poc, not a DEVIATE: `ConstantFoldLoadFromConst` and
`ConstantFoldLoadFromConstPtr` (ConstantFolding.cpp). Until they land,
`analyze_load_from_clobbering_mem_inst` of a `memcpy`/`memmove` (which
needs them to accept) is none, `get_constant_value_for_load` is none, and
`get_constant_mem_inst_value_for_load` folds only into an integer type (the
splat itself). Each is a load GVN then leaves in place.

## Example
A word stored, its upper half loaded. Before:

```mir
store i32 %x, ptr %p
%q = getelementptr i8, ptr %p, i16 2
%v = load i16, ptr %q
```

`analyze_load_from_clobbering_store` says the load starts at byte 2 of the
store, and `get_value_for_load` builds what it reads:

```mir
%s = lshr i32 %x, 16
%v = trunc i32 %s to i16
```

From unit test `test_a_half_of_a_word_is_shifted_and_truncated` in
`vncoercion_tests.rs`.

## Used by
Nothing yet; `gvn`'s load elimination (stage 4 of the GVN port) is the
caller this is for.
