> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/functionattrs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/functionattrs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/functionattrs.rs sha256:22f265921924edbd30de354f204b14cfc5bde93af9bacbec42cc5f980cee274a -->

# functionattrs

`norecurse`, `nounwind`, `willreturn` and `noreturn` of one call-graph SCC,
as LLVM's `PostOrderFunctionAttrs` infers them from a body and the
attributes of its callees.

## How it works
`infer` takes the members of an SCC, callees already stated, and answers
what each may be stamped with. It reads no analysis: a call is judged by
its site's attributes and its callee's declaration.

- `norecurse` (`addNoRecurseAttrs`): a one-member SCC, every call of which
  goes to a function other than itself that states `norecurse` or is a
  `nocallback` declaration or an intrinsic (an indirect call with `!callees`
  counts each listed target, DEVIATE F2).
- `nounwind` (`inferAttrsFromFunctionBodies`): all or none of the SCC. No
  member may be a declaration or inexact; no instruction may throw (a call
  without `nounwind`, an `invoke`, `resume`) except a `call` to another
  member.
- `nofree` and `nosync` (`InstrBreaksNoFree`, `InstrBreaksNoSync`): every
  call states it or reaches an SCC member; all or none of the SCC.
- `willreturn` (`addWillReturn`): `mustprogress` and no writes, or no back
  edge and every call states `willreturn`.
- `noreturn` (`addNoReturnAttrs`): no path from the entry reaches a `ret`
  without a call that states `noreturn` in its block.

## Example
A function that calls a `norecurse` function and adds is itself
`norecurse`, `nounwind` and `willreturn`:

```mir
declare i32 @leaf(i32) norecurse nounwind willreturn

define i32 @f(i32 %x) {
b0:
  %a = call i32 @leaf(i32 %x)
  %b = add i32 %a, 1
  ret i32 %b
}
```

From unit test `test_a_body_over_norecurse_callees_is_stamped_all_three` in
`functionattrs_tests.rs`.

- `memory(...)` (`addMemoryAttrs`, `checkFunctionMemoryAccess`): the meet
  over the SCC of what each body accesses, by the places its instructions
  name; a call is judged by `AAResults::getMemoryEffects`, a location by
  `getModRefInfoMask` and its underlying object. Calls inside the SCC are
  free, except that their pointer arguments count if the SCC reads argument
  memory. DEVIATE F3: a `noipa` body is left out of the SCC (it is this
  repository's `optnone`: callers read what the source declared). DEVIATE
  F4: no `writable` argument attribute to drop (MIR has none).

## Runs
`interprocedural::stamped` calls `post_order` once per SCC, callees first:
`memory(...)`, the pointer parameters' attributes, then the four flags.
DEVIATE F1: `willreturn` also where a loop's trip count is proved or the
language promises each loop ends, in a function that does nothing
observable (`will_return_by_loops`).
Exit: once llrm-cc over clang is the only C frontend its IR carries
`mustprogress`, and LLVM's rule gives the same answer; F1 goes then.

## See also
LLVM `FunctionAttrs.cpp` (`addNoRecurseAttrs`, `addNoReturnAttrs`,
`addWillReturn`, `inferAttrsFromFunctionBodies`), `CFG.cpp` (`canReturn`).
