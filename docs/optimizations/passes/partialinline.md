> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/partialinline.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/partialinline.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/partialinline.rs sha256:90e886ffd59d0efc11a9e60e0141f18e9c491d55ab730458b080ee112ec06e4c -->

# partialinline

PartiallyInlineLibCalls: a call to `sqrt` or `sqrtf` that may set `errno`
becomes the target's square-root instruction, and the library call runs
only when the instruction cannot answer (a NaN, a negative operand)
([PartiallyInlineLibCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/PartiallyInlineLibCalls.cpp)).

## How it works
A call is a candidate when the library has the function, the target answers
`have_fast_sqrt` for its type, and the call is not `nobuiltin`, `strictfp`,
`musttail`, to a local function, or already read-only (the back end emits
the instruction for that one). The call is marked `memory(none)`; a branch
on `fcmp ord v, v` (`fcmp oge x, 0.0` where the target says that costs
less) skips a clone of the call, and a phi joins the two results. One call
per block: the rest of the block is the next block, visited next.

```text
 b:  %r = call double @sqrt(double %x)       b:  %r = call double @sqrt(double %x) memory(none)
     ...                                         %c = fcmp ord double %r, %r
                                                 br i1 %c, label %b.split, label %call.sqrt
                                             call.sqrt: %l = call double @sqrt(double %x)
                                                        br label %b.split
                                             b.split: %p = phi double [%r, %b], [%l, %call.sqrt]
                                                      ...
```

Why it matters here: a `sqrt` that sets `errno` is a call; with this it is
an instruction on the path that has no error.

DEVIATE PIL-OS: where the target's `Tuning::partial_libcalls_skip_at_size`
is on (x86, arm64, wasm), a function with `optsize` or `minsize` is
skipped. The pin splits there too, which grows -Os code by a compare, a
branch and a copy of the call, and -Os is strict bytes. Owner: alim; exit
2026-11-27, revisit when the -Os rows allow.

DEVIATE PI1: the likely-branch weight the pin adds when the function has an
entry count is not added: MIR has no profile counts.

## Runs
After the strength slot (the pin: `addIRPasses`, after LSR), before the
code-generation-prepare passes.

## Example
A `sqrt` call that may set `errno` becomes a `memory(none)` call that the
target turns into an instruction, with the real call kept for the NaN or
negative case.

```text
%r = call double @sqrt(double %x) memory(none)
%c = fcmp ord double %r, %r
br i1 %c, label %b.split, label %call.sqrt
call.sqrt: %l = call double @sqrt(double %x)   ; joined by a phi in %b.split
```
