# Casts through the selector space: audit

The selector-alone space (`[spaces].segment`, `ni:` in the datalayout) drops the offset: near -> selector keeps the segment, selector -> far gives segment:0. A pair of casts through it is not one cast. Who composes or strips addrspacecasts (2026-10-21, `grep AddrSpaceCast`):

| Site | What it does | Verdict |
|---|---|---|
| `constantfolding.rs` `is_eliminable_cast_pair` case 12 (callers: `constant_fold_cast_operand`, `combine_casts.rs`) | composes addrspacecast pairs (pin Instructions.cpp#L3043) | fixed: DEVIATE C1, no fold through an `ni` middle |
| `isel.rs` `global()` | a constant selector cast back to far was lowered as the global's own far address (offset kept) | fixed: selector:0 |
| `constantfolding.rs` offsets/`get_underlying_object`, `valuetracking.rs`, `valuetracking/underlying.rs`, `memory.rs` (MemRef roots), `basicaa.rs`, `globalopt.rs`, `switchlookup.rs` | strip addrspacecasts to name the underlying object | unsafe in principle: a far pointer made from a selector is named by the global it came from; not fixed here, row to follow |
| `alias.rs:3332,3381` | stops at the segment space | safe |
| `frameescape.rs`, `globalsmodref.rs`, `capturetracking.rs`, `capturecheck.rs` | follow uses through casts | safe (they over-approximate escapes) |
| `globals.rs` `target()` (data initializers) | looks through casts for the global an address names | unsafe for a far pointer initialized from a selector; not reachable from QB output today |
| `sroa.rs`, `mem2reg.rs`, `memcpyopt.rs`, `deadstore.rs`, `spill.rs`, `tailcallelim.rs`, `splitcopy.rs`, `combine_loadstore.rs` | look through casts on accesses | not audited one by one: they follow a pointer to its access, not its identity |

How the symptoms became visible: the pair became foldable with the ConstantFolding translation (4a2b1b423a, 2026-10-18) and visitTrunc (bae70570c1). far3's wrong output was masked by a compile failure fixed by cb2ce8f3e; far1's compile failure appeared with fbbe81205 (#3262), which unmasked the folded shape.

## Rows above main

| Row | What | Owner | Exit | Review |
|---|---|---|---|---|
| FORGED-SEL-1 | deedlines -Os +201 B (0.6%) above main and above base: a forged selector (DEF SEG = VARSEG, POKE) leaves no global tracked, so loads and stores of the program's globals are kept in memory | funcattrs-port | prove non-aliasing where the selector provably is not DS | 2026-11-30 |
