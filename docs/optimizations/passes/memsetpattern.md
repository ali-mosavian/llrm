> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/memsetpattern.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/memsetpattern.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/memsetpattern.rs sha256:4cd35c9ea6f87136997f10537202f11b06e1260e5c28d17e7a823914a7721778 -->

# memsetpattern

Expands `llvm.experimental.memset.pattern` into a store loop in MIR before
instruction selection, as the pin's `PreISelIntrinsicLowering` does (the
DAG never sees the intrinsic).

## How it works
The block is split after the call. A constant length of zero makes
nothing; a constant one jumps to the loop, a variable one first tests
`len != 0`. The loop body stores the pattern at `dst[i]`, steps `i` and
repeats while `i + 1 < len`. One element per iteration: x86 asks for no
wider loop type (`getMemcpyLoopLoweringType` is `i8`).

## Example
`call @llvm.experimental.memset.pattern(ptr %p, i16 1824, i32 %n, i1 false)`

```text
  %c = icmp ne i32 %n, 0 ; br %c, main-body, post-expansion
main-body:  %i = phi [0, pre], [%j, main-body]
            %a = getelementptr inbounds i16, ptr %p, %i ; store i16 1824, %a
            %j = add %i, 1 ; %k = icmp ult %j, %n ; br %k, main-body, post
```

## Runs
Once per module in `driver::optimized`, for `--isel=dag`: the old selector
lowers the intrinsic itself.

## See also
LLVM's `createMemSetPatternLoop`
([LowerMemIntrinsics.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/LowerMemIntrinsics.cpp#L1304)).
