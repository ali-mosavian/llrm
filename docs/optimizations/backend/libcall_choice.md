> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/libcall_choice.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/libcall_choice.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/libcall_choice.rs sha256:c2b8ec6c6afa9d0bb637c2ee53e08d8384f108c7d24084e1e5bbdefcc10b95d4 -->

# libcall_choice

Which operations a size-built module calls a runtime routine for, and which
it expands inline, by the bytes of the whole linked program.

## How it works
A target describes each operation that can go either way as a
`Candidate`: the key and the runtime routine it calls. A routine is a
`Member`: its bytes, and the members it calls. The unit measures what
calling would save: per candidate, the bytes of its inline sites less the
bytes of the same sites as calls (a site's inline code varies with what is
known of its operands, so no table column prices it). `choose` takes
those savings and returns the candidates to call: the set whose savings,
less every member they link once, are the greatest and positive. A tie is
inline.

DEVIATE M68K-SIZE-LIBCALL: the pin's `Expand` against `LibCall` is a fact of
the operation's legality (`setOperationAction`) with no accounting of the
library, so it picks one for every function. Reason: best of gcc and clang
is the goal at `-Os`, and the bytes of a call are the bytes of the routine
too. A member is charged in full to this translation unit's sites: a unit
cannot know the uses of another (conservative). Sizes come from the target
description. Owner m68k. Review: 2026-12-20.

## Runs
Called by a target's libcall table (`llrm-m68k` `libcalls.rs`) at `-Os`,
once per module.

## Example
Each candidate carries what calling it saves; `choose` calls the set whose
savings, less the routines it links once, are the greatest and positive
(the byte sizes are illustrative).

```text
mul: saves 40 bytes, calls __mulsi3 (20 bytes)  -> 40 - 20 > 0  call
div: saves 30 bytes, calls __divsi3 (50 bytes)  -> 30 - 50 < 0  inline
```
