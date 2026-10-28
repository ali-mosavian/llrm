> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/forging.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/forging.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/forging.rs sha256:5ec22c47566f7c06306f3327c91b48af3f0f7b9692ec1867457ac27e2b18bddf -->

# forging

Whether a module forges pointers into the data segment: the one fact that
decides if a global can be "reached by name only" on a segmented target.

## How it works
The pin's `GlobalsAAResult` (and every analysis that reads it) takes a
global whose address is never computed as unreachable by a pointer of
unknown origin: forging a pointer to it is undefined
([`isNonEscapingGlobalNoAlias`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/GlobalsModRef.cpp#L813)
answers may-alias for a pointer from `inttoptr` only because it cannot
point there). Real-mode BASIC defines it: `DEF SEG = VARSEG(w)` and `POKE`
reach any byte of the data segment, a variable whose address nobody took
among them. A far pointer is forged where an `inttoptr` makes a selector
alone (the segment space the target description names) from an integer that
is not a plain number.

A plain numeric selector (`&HB800`) is exempt: it names a segment no
program object occupies. The data segment's own value is a load-time
relocation, never a number a source can write, so a constant cannot name
it; a selector computed (`VARSEG`, arithmetic, a `PEEK` result) is not
exempt.

A selector read from an array descriptor (an `inttoptr` the frontend marks
`!llrm.descriptor`) is exempt too: it is the array's own segment, which an
access in bounds never leaves.

The runtime's own code is no forger of the program's: its bodies are
`available_externally`, and the runtime module is another module; both are
out of the scan, the same trust boundary as `!llrm.named`.

## Example
`%s = add i16 %v, 4` then `%p = inttoptr i16 %s to ptr addrspace(2)` forges:
the selector is computed, and every global of the module is reachable by it.
`inttoptr i16 -18432 to ptr addrspace(2)` (`&HB800`) does not.

## Provides
`forges_selector`; the manager entry `ForgedSelectors` computes it once for
a module, and `GlobalsAA` and `GlobalsModRef` take it from there.
