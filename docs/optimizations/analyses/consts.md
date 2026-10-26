> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/consts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/consts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/consts.rs sha256:08ecdbaf394e116c9fa60a731af157f55f6bc37e797ed19fedeadc1c3decf038 -->

# consts

Which values are known, and what they are: integer constant propagation and
folding, with memory read through MemorySSA.

## How it works
A fact is `Known { n, width }`: the low `width` bits of the value are `n`,
nothing is said above them. An operation folds only where the widths
agree (`_result`): integer `add`, `sub`, `mul`, `and`, `or`, `xor`, shifts,
divisions that do not fault (`folds`), `sext`, `zext`, `trunc`, `icmp`
(`holds`), `x ^ x` and `x - x`, the fixed-point intrinsics, and a pointer
compare of two addresses of one global. A width is in bits; a memory
cell's, in bytes.

Registers. `known` runs a fixed point over the body in layout order: each
integer result not yet known is folded from the facts so far, and a phi
is known where every incoming value is known and they agree. It then
hands the facts to [`crate::constant_cycles`](constant_cycles.md), which settles the phis whose
incoming values depend on each other around a loop.

Memory. `known_walked` starts from the register facts and, for each load,
asks [`crate::memoryssa`](memoryssa.md) which stores reach it (`load_providers`). If
they all put the same number in the bytes the load reads, so does the
load (`provided`; a store may serve a load of another type, or several
stores serve one byte at a time). No state is held per block, so the cost
is the walks, not cells times joins. A load whose index only memory taught
is asked again with the index placed, up to four rounds. `known_walked_over`
adds what counted float loops leave in memory at their exits
([`crate::floatfacts`](floatfacts.md)).

```text
   store i16 7, @g          known:        %a is not known
   %a = load i16, @g        known_walked: %a = 7, %b = 8
   %b = add i16 %a, 1       (the walk finds the store)
```

The caller states what each call writes (`Calls`, from
`alias::calls_by_attributes`); a call not listed writes what
`memory::unmodeled_write` says. A far store through a segment selector
nothing yet resolves is taken to write no program object, and the solve is
made again without any such selector that did not resolve to a number.

`memory_queries` and `_MemoryQueries` answer alias questions about the cells
of one solved epoch, each cell carrying its object, for passes that walk
instructions with a fact table ([`crate::cellmap`](cellmap.md) indexes the cells by
overlap bucket).

## Example
A load of a global that a constant-index store wrote:

```mir
@g = global [128 x i8] zeroinitializer

define i16 @f() {
b0:
  %index = add i16 2, 2
  %p = getelementptr i8, ptr @g, i16 %index
  store i16 9, ptr %p
  %loaded = load i16, ptr getelementptr (i8, ptr @g, i16 4)
  ret i16 %loaded
}
```

| Value | `known` | `known_walked` |
| --- | --- | --- |
| `%loaded` | none | `Known { n: 9, width: 16 }` |

From unit test `a_store_through_a_constant_index_is_read_back` in
`consts_tests.rs`.

## Provides
`Known`, `known` (registers, cached as [`crate::manager::Registers`](manager.md)),
`known_walked` and `known_walked_over` (memory; cached as
[`crate::manager::ThroughMemory`](manager.md)), `memory_queries`, `holds`, `folds`.
Consumers: `gvn`, `algebraic`, `fold`, `decide`, `unswitch`, `floatloop` and
`interprocedural` in `llrm-transforms`, the backend's `isel`, and most
analyses of this crate.

## Needs
[`crate::memoryssa`](memoryssa.md), [`crate::alias`](alias.md) (the call table), [`crate::ranges`](ranges.md),
[`crate::regions`](regions.md), [`crate::cellmap`](cellmap.md), [`crate::constant_cycles`](constant_cycles.md).

## See also
LLVM `SCCP`/`ConstantFolding`; GCC FRE walks the same virtual use-def chain
for a load's value (`vn_reference_lookup`). `LLRM_CHECK_CYCLES` in
[`crate::constant_cycles`](constant_cycles.md).
