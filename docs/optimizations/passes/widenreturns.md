> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/widenreturns.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/widenreturns.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/widenreturns.rs sha256:7ca71f8306dacbf188fc27ae69209760611606777ae3d1d10d63b7e125acdbb3 -->

# widenreturns

Return-value widening: a function only its own module calls, whose every
caller extends the result to one wider type, returns the wider value
itself. The extension is made once where the function returns, not at each
call.

## How it works
Only functions `direct_only` accepts are candidates (internal, never named
but as a callee, every call a plain call). One qualifies when it returns an
integer and every call reads the result only through casts of one kind
(`zext` or `sext`) to one type. Then each `ret` returns the value cast to
that type, the function's type follows, and each call returns the wider
type: its casts read the call's result and are gone. The return and every
call are marked `zeroext` (`signext`), the ABI's own statement that the
callee extended: a call lowering that reads the returned registers as they
stand (the `AssertZext`/`AssertSext` of `lowerRangeToAssertZExt` and
`getCopyFromParts` are skipped for an attribute that already says so) does
not rebuild the high part from a `range` it infers.

## Example
```text
 define internal i16 @f(i16 %n)        define internal i32 @f(i16 %n)
   ret i16 %v                  ==>       %w = zext i16 %v to i32
 %r = call i16 @f(i16 %a)                ret i32 %w
 %w = zext i16 %r to i32               %w = call i32 @f(i16 %a)
```

It pays where the calls outnumber the `ret`s: the extensions move from the
calls to the returns. A target decides whether a wide return is cheap
(`Tti::widen_private_returns`).

## Not in the pin
Nothing in LLVM changes a function's return type to a wider one:
DeadArgumentElimination
(`DeadArgumentEliminationPass::removeDeadStuffFromFunction`) drops dead
returns and arguments, IPSCCP (`IPSCCP.cpp`) propagates constant returns and
ranges without retyping, ArgumentPromotion changes parameters only, and the
Attributor deduces `zeroext` and `range` of the return without removing the
caller's cast. GCC's nearest is IPA-SRA's return-value rewrite (`ipa-sra.
cc`, `ipa_param_adjustments`), which narrows or removes a return, never
widens it. `DEVIATE WIDEN-RETURNS`.

## Runs
In the interprocedural step, after dead-argument elimination, on a target
that opts in. Each changed body goes through the function pipeline again.
