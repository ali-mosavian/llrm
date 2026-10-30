> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/globalsmodref.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/globalsmodref.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/globalsmodref.rs sha256:ea6639b476c55a5edd1fb13d2a893b029e8d6a94aa88782c6ea48ec4a5a7ddc9 -->

# globalsmodref

What each function does to the globals no code reaches but by name: LLVM's
`GlobalsModRef.cpp` (`GlobalsAAResult`), translated at the pin
(<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/GlobalsModRef.cpp>).

## How it works
`AnalyzeGlobals` finds the internal globals whose address is only loaded
from, stored through, compared with null or passed to a `nocallback`
declaration (`AnalyzeUsesOfPointer`), with the functions that read and
write each. `AnalyzeCallGraph` then walks the call graph callees first, one
SCC at a time: a function gets a `FunctionInfo` (one `ModRefInfo` for all
memory, a `ModRefInfo` per tracked global, and "may read any global") only
if every callee of the SCC has one. A call to a declaration that may call
back into the module (no `nocallback`), an indirect call or a function
whose definition is not exact leaves the caller without. `get_mod_ref_info`
answers a call against a location that is a tracked global; any other
location is left to the caller's other analyses.

```text
 @g = internal global i16 0      f reads @g          g() calls f, writes @g
 define i16 @f() { load @g }     f: {@g: Ref}        g: {@g: ModRef}
 define void @g() { call f; store @g }
```

## Example
A call of `@f` can read `@g` and not write it:

```mir
@g = internal global i16 0

define i16 @f() {
b0:
  %v = load i16, ptr @g
  ret i16 %v
}
```

`info(@f).mod_ref_for_global(@g)` is `Ref`.

From unit test `a_reader_of_a_private_global_has_ref_on_it` in
`globalsmodref_tests.rs`.

## Deviations
- G1: `IndirectGlobals` and `AllocsForIndirectGlobals` (the `alias` rule for
  a global that owns heap memory) are not built; it only adds `NoAlias`
  answers, so leaving it out is conservative.
- G2: an indirect call with a `!callees` list is the edges it lists (llrm's
  `CallGraph`); LLVM's call graph calls external.
- G3: `NullPointerIsDefined` is taken as true: a null pointer is never
  `NoAlias` with a tracked global.
- G4: no `TargetLibraryInfo`: a call to `free` is not recognised, so a
  global passed to it is not tracked.
- G5: `hasOptNone` is `noipa` (`llrm_mir::facts::opaque`).
- G6: `escaping_among` (GlobalsAA's use) does not count a global named in an
  initializer as escaped; GlobalsAA reads an initializer only through its
  holder (`globalsaa::held`). QB's `REDIM` descriptor held every array's
  address in a constant nothing read: deedlines +128 memory operands without
  it
  (test
  `a_private_initializer_nothing_names_does_not_leak_the_address_it_holds`).
  Exit: the pin's rule, once a rows pair shows no row worse.
- G7: `llrm.ia16.window` and `llrm.ia16.restrict` return their operand's
  bytes, so the scan follows the call's result as it follows a `gep`, as the
  pin follows `threadlocal.address`.
- G6: a declaration needs `nocallback` only, not `nosync` as well
  (`MaySyncOrCallIntoModule`): MIR has no `nosync` and the targets no
  threads.

## Provides
`GlobalsModRef`, `FunctionInfo`, `compute`.

## Needs
[`llrm_mir::callgraph::CallGraph`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/callgraph.rs), [`crate::aa`](aa.md) (`ModRefInfo`,
`MemoryEffects`).

## See also
[`crate::globalsaa`](globalsaa.md), which finds the same globals from `alias`'s
summaries and is what the passes read today.
