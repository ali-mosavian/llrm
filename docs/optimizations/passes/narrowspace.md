> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/narrowspace.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/narrowspace.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/narrowspace.rs sha256:8f568aeb453a3e6c4a0ba55b3604c9e0388b656a040d854d8df34800e9c91ad1 -->

# narrowspace

A far pointer parameter that every call fills from one near space becomes
a near pointer parameter: the caller passes the offset, and neither
segment is pushed or loaded.

## How it works
A parameter is a candidate when its function is reached only by direct
calls (`callgraph::direct_only`, none of them refused) and the parameter
is a far pointer that does not carry (a pair, not a huge one). For each
call the actual's root (`valuetracking::underlying`) says which near space
the object is in:
- a module global variable in the data space (DGROUP's, `Spaces::data`);
- a stack object of the caller (SS, which DS need not be: `Spaces::stack`);
- a near parameter an earlier round narrowed, of that parameter's space;
- the function's own parameter passed on in a recursive call: no evidence
  either way.

The parameter is narrowed when at least one call names a space, every one
that does names the same, and every other call is a pass-on. An actual that
may be elsewhere, or in the other space, keeps the parameter far. Rounds
repeat until nothing narrows, so a callee's narrowing feeds the callers that
pass its parameter on.

```text
 callee(ptr addrspace(1) %a)          callee(ptr %a)
   ... load %a ...                      %w = addrspacecast %a to as(1)
                                        ... load %w ...
 caller: call callee(%far)    =>      caller:
   %far = addrspacecast @g to as(1)     %n = addrspacecast %far to ptr
                                        call callee(%n)
```

The callee's new near parameter replaces the old one; what read the old one
reads a cast of it to the far pointer, and the recursive calls that passed
it pass the near one. Each other call passes a cast of its actual to the
near pointer. [`crate::inferspace`](inferspace.md) then folds the accesses in the callee
and the casts at each call, so the segment is neither pushed nor loaded.

## Example
Before:
```mir
define ... @"_next@3"(ptr addrspace(1) nocapture readonly %0, i16 %1) ... {
b1:
  %2 = mul i16 %1, 2
  %3 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 %2
  %4 = load i16, ptr addrspace(1) %3, !tbaa !9
  ret i16 %4
}

define ... @"_walk@3"(i16 %0) ... {
b1:
  %1 = addrspacecast ptr @_table to ptr addrspace(1)
  %2 = call regparm3cc i16 @"_next@3"(ptr addrspace(1) %1, i16 %0)
  %3 = call regparm3cc i16 @"_next@3"(ptr addrspace(1) %1, i16 1)
  %4 = add nsw i16 %2, %3
  ret i16 %4
}
```
After:
```mir
define ... @"_next@3"(ptr nocapture readonly %0, i16 %1) ... {
b1:
  %2 = mul i16 %1, 2
  %3 = getelementptr inbounds i8, ptr %0, i16 %2
  %4 = load i16, ptr %3, !tbaa !9
  ret i16 %4
}

define ... @"_walk@3"(i16 %0) ... {
b1:
  %1 = call regparm3cc i16 @"_next@3"(ptr @_table, i16 %0)
  %2 = call regparm3cc i16 @"_next@3"(ptr @_table, i16 1)
  %3 = add nsw i16 %1, %2
  ret i16 %3
}
```
Both calls pass the near `@_table`, so `_next@3` takes a near pointer: the
casts and the segment the far parameter carried are gone from the callee and
from each call.

From `--dump` of a program whose static `next(int far *p, int n)` is called
twice with the near global `table`, at -O2 -m16 with inlining off, stages
`pipeline` and `interprocedural`.

## Runs
In [`crate::interprocedural`](../analyses/interprocedural.md) as stage `narrow.`, after argument promotion
and before the constant-return rounds; the functions it changes go back
through the per-body pipeline. On wherever the interprocedural step is
(every level above -O0).

## Needs
The target's `Spaces` (data and stack roles) and its address-space layout.

## See also
[`crate::argpromotion`](argpromotion.md), [`crate::deadargs`](deadargs.md).
