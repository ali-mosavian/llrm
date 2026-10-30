> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/references.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/references.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/references.rs sha256:bd322128469574e801032a3eb7f25cec29b21d722261e3c75ab7b46881607563 -->

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

## Example
Two loads whose pointers are `gep`s of one `alloca`; `%q`'s offset is then
rewritten:

```mir
define i16 @f(i16 %i) {
b0:
  %a = alloca [8 x i16]
  %p = getelementptr inbounds i16, ptr %a, i16 %i
  %q = getelementptr inbounds i16, ptr %a, i16 3
  %x = load i16, ptr %p
  %y = load i16, ptr %q
  %z = add i16 %x, %y
  ret i16 %z
}
```

| ask | decomposes |
|-----|------------|
| `%x`, `%y`, `%x`, `%y` | two: the repeats read the kept ones |
| after `%q` is rewritten, `%x` and `%y` | one: `%y`, whose chain holds `%q` |

From unit test
`test_a_changed_pointer_chain_decomposes_again_and_an_unchanged_one_does_not`
in `references_tests.rs`.

## Provides
`of` (what `MemRef::of` reads through), `built` (decompositions made on this
thread, for the test that a second ask makes none).

## Needs
[`crate::memory`](memory.md) (`MemRef::decomposed`).

## See also
LLVM's `MemoryLocation::get`, computed per access and kept by its users;
`BasicAA`'s `DecomposedGEP`.
