> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/references.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/references.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/references.rs sha256:6aec689106581203bfb597e09c267bfdb0ef6a7b6063ed5dde6fc1d6126e828e -->

# references

The base reference of each load and store, decomposed once and shared.

## How it works
`MemRef::of` splits an access's pointer into a root, a constant
displacement and one scaled index, and reads its `!tbaa` type: a fact of the
instruction and the pointer chain under it, which the points-to solves,
`Annotated`, `Accesses`, the walker and the passes each asked again, 181
times in a d_faces -O1 compile (`MemRef::of` was 6.5% of it). Here each
access is decomposed on its first ask and kept as an `Rc<MemRef>`, with the
values its chain read.

The table is held by the module's `Context` (as `typed`'s names are), one
for each function history (`Mark::lineage`; a copy of a function has its
own). Asked after the function changed, it reads the change log since its
mark and drops the references the changes reach: an access whose operand was
rewritten or erased, and each access whose chain holds a value whose
definition was. A history it cannot read is dropped whole, as is one made
under other metadata. Moves change nothing: a reference does not read where
an instruction stands.

```text
 load p ← gep a, 4 ← alloca a         chain of the load: p, a
 gep operand rewritten (Operand, Rewritten)  → drops the load; the next ask
                                              decomposes it again
 an unrelated add rewritten                   → keeps it
```

`LLRM_CHECK_REFERENCES=1` decomposes afresh on every ask and asserts the
kept one equal.

## Provides
`of` (what `MemRef::of` reads through), `built` (decompositions made on this
thread, for the test that a second ask makes none).

## Needs
[`crate::memory`](memory.md) (`MemRef::decomposed`).

## See also
LLVM's `MemoryLocation::get`, computed per access and kept by its users;
`BasicAA`'s `DecomposedGEP`.
