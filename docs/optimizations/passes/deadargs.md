> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/deadargs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/deadargs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/deadargs.rs sha256:9ff504ddd26456f4d343dc14f11b16b7669d2caae621266bc8ebb3ff04929e90 -->

# deadargs

Dead argument elimination (LLVM's DeadArgumentElimination): a parameter
nothing reads goes, and with it the argument at every call.

## How it works
Only functions `direct_only` accepts are candidates (internal, never named
but as a callee, every call a plain call with matching arity).

A parameter is live when anything but a recursive call reads it. A use as an
argument of the function's own call is live only if the parameter it lands
on is live, taken to a fixed point. So `hanoi(n - 1, a, c, b)`, which
permutes three parameters among themselves and reads none, loses them all.

```text
 f(i16 %len, i16 %p)            f(i16 %p)
   ... %len unread       ==>      ...
 call f(12, %q)                 call f(%q)
```

Each dead parameter is removed from the function and the argument from every
call, in the function itself and its callers. Constant propagation and
promotion are what leave these behind: a length every caller passes as 12,
folded into the body, is still pushed at every call.

## Runs
In the interprocedural step, after the callers' arguments have been
propagated as constants and ranges, and before recursive inlining. Each
changed body goes through the function pipeline again. Not at -O0.

## See also
[`crate::argpromotion`](argpromotion.md); LLVM's DeadArgumentElimination (`MarkValue`,
`SurveyUse`).
