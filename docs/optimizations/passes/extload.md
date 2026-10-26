> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/extload.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/extload.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/extload.rs sha256:57f8b70a7e431d6d8b7903f633a2c7bbe969d16c0d899a1e59143998e293d735 -->

# extload

One extending load for a narrow load and its extension: LLVM's
DAGCombiner
[`ExtendUsesToFormExtLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15365)
and
[`ExtendSetCCUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15421),
as the rewrite of MIR before isel.

## How it works
For a load whose value `E = sext/zext L` is made in the same block, the
compares of `L` with a constant, in that block or any block the load
dominates, compare `E` instead, with the constant extended the same way;
the same extension made again is `E`, and a reader in the load's own block
reads `trunc E`; any other reader leaves the load alone. `L`
is then read only by `E`, and isel selects the pair as one `movsx`/`movzx`
from memory: the compare is on the wide register, not on a byte register
that `E` must extend again.

A `zext` is not made where a signed compare reads `L` (its sign bits would
be lost). `E` is moved to just after `L`, so that it precedes the compares
it now feeds.

| # | DEVIATE |
|---|---|
| D1 | Done on MIR before isel, not in the DAG: isel selects one instruction at a time (`isel/combined.rs`) and has no combine stage. Owner mir-undef; revisit when isel gains one. |
| D2 | A reader of the load that is neither a compare with a constant, the same extension again nor in the load's own block (a store or a call in another block) leaves the load alone. The pin gives it `trunc E` (`isTruncateFree`); there the narrow value lives beside the wide one: bench/grep.nib `_main` grew 58 B of spills. Owner mir-undef; revisit when isel can fold the trunc. |
| D3 | A compare with a constant in a block the load dominates, not only in its own, is widened (the pin's DAG is one block): x_life -O2 727 -> 711 B, -O3 the same, m16 -O1 668 -> 661, -O2 644 -> 641. Cost: bench/grep.nib is +33 / +29 / +25 B over parity at -O1 / -O2 / -Os (and 115 / 173 / 159 B under main): `_main` spills one more range (greedy trace: spilled 31 -> 32, stores 8 -> 12). The cause is an UNTESTED hypothesis: a long range that moves from the byte class to the word class can take SI or DI, which grep uses as base registers. render/d_alias is above main already (-O2 12522 main, 14921 parity; -Os 11836, 12232); the rule adds +12 B at -O2 (0.08%: bytes do not decide there) and +2 B at -Os (0.016% of 12232, under the 0.05% bar): nothing beyond those bytes. Exit: a pressure-aware gate for the cross-block case. Owner mir-undef; review 2026-11-14. |

## Example
Before:

```mir
b0:
  %c = load i8, ptr %p
  %lt = icmp sgt i8 %c, 0
  %w = sext i8 %c to i16
  br i1 %lt, label %b1, label %b2
```

After:

```mir
b0:
  %c = load i8, ptr %p
  %w = sext i8 %c to i16
  %lt = icmp sgt i16 %w, 0
  br i1 %lt, label %b1, label %b2
```

From unit test `test_a_compare_of_a_loaded_byte_compares_its_extension` in
`extload_tests.rs`.

## Runs
Once per body, after `cgpaddr`, where the other passes that make the
forms isel sees run (the pin has none: its DAGCombiner does it). With
`wanted("extload")`.

## Needs
Nothing.

## Keeps
Blocks, dominators and loops.

## See also
Tests: `extload_tests.rs`. Levels: `docs/reference/levels.md`.
