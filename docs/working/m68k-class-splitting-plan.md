# m68k class splitting: scoping plan (2026-11-24, no code)

Data: QB `.text`, 68020 -Os, release/next 6f0098abc3 + #10797 (46c7b3f6cf); 49 modules over gcc-15 (+6,404 B), 123 comparable. Instrument: objdump of shipped objects, both bases (`sp@` and gcc's `fp@`).

## (a) What the pin does, what ours has
- Ours already is the pin's allocator: m68k allocates with `allocate_greedy_after` (`llrm-m68k/src/regalloc.rs:292-331` -> `llrm-spike-mf` `tolir.rs:3118`) over the translated RegAllocGreedy (`llrm-codegen/src/greedy/`: `splitkit`, `regionsplit`, `spliteditor`, `liverangeedit`, `inlinespiller`, `hoistspill`, 18k lines) and the translated RegisterCoalescer (`coalescer/`). The graph-colouring "no spilling" allocator (`llrm-spike-mf/src/regalloc.rs`) is the spike; `addrarith.rs`'s "this allocator does not split" dates from it and is stale.
- m68k hooks on the shared allocator, all DEVIATE: `joins_sub_registers` false (C13), `joins_global_copies` (JoinGlobalCopies, built both ways), `joins_loop_carried_first` (C15), `Class::meet`, `candidates` order.
- The pin does NOT choose a class by cost. isel gives each vreg a class from its definition (`ADD32dd` is DR32, `MOV32jr` base is AR32); `constrainRegClass` narrows the vreg to the intersection of its uses and inserts a `COPY` for a use that cannot take it; greedy splits live ranges, never changes a class demand. The d<->a copies are in the pin's output too (pin llc is no byte oracle at m68k; DEVIATE passes already cover what gcc does).
- gcc picks a class per pseudo family by cost (`regclass.cc` preferred/alternate class: sum of use costs per class) and reloads only the exceptions.
- Our private class passes (the m68k "class choice" DEVIATE sites): `addrroles.rs` (base/index roles), `classsplit.rs` (hold in D across a loop), `addrarith.rs` (lea/addq/adda; #10797 widens it), `addrcmp.rs` (cmpa), `moveqremat.rs`, `slotreg.rs`. None of them is missing a pin pass; each approximates gcc's regclass for one shape.

## (b) What is left, in bytes (excess copies x 2 B, ours minus gcc, shipped objects)
| copy | all 123 | over-50 | bytes (all / over-50) |
|---|---|---|---|
| d->a `moveal %d,%a` | +1023 | +744 | 2.0 KB / 1.5 KB |
| a->d `movel %a,%d` | +198 | +231 | 0.4 KB / 0.5 KB |
| d->d `movel %d,%d` | +1593 | +1065 | 3.2 KB / 2.1 KB |
| a->a | -41 | -12 | 0 |
Copy excess is 5.6 KB / 4.1 KB: an upper bound (gcc needs copies too), 64% of the over-50 gap.
Shapes (4 functions, 495 surviving copies; per-shape counts from the dump, liveness-checked): class crossing 44% (126 of 220 dead-source = pure class conversion), d->d 29% (121 of 145 with the source still live: a duplicated value, same class, not a class problem). Crossing by definition of the d value: defined in another block / phi 49%, data add 25%, load into d 11%, shift (index scale) 7%, rest 8%.
Per shape ceiling (copy bytes only), over 123 / over-50: cross-block class ~0.7 / 0.7 KB (measured excess 366 / 349 copies); load into d (a `movea` load) ~0.25 / 0.1 KB; data adds ~2.2 / 1.4 KB, of which #10797 took 1.02 KB (the sweep is -1,020 B); shifts ~0.2 KB. d->d live-source copies: 3.2 / 2.1 KB, but class splitting cannot reach them.

## (c) Design if pursued
- Cheaper than a splitter: gcc's regclass as an isel-time pass: union-find over copy/phi webs of a vreg family, cost(An) vs cost(Dn) = sum over uses of the byte/clock gain of the form each class allows (addressing modes need An; ALU/shifts/cmp-with-imm need Dn; moves either), pick the class per web, copies only at the boundaries. One pass over uses: O(n); no change to greedy. It replaces the shape-by-shape rewrites (`addrroles`, `classsplit`, the `Use::Either` rule of #10797) by one general rule; per-function trial at -Os/-O1 as #10797.
- Cost model: copy 2 B (move.l, 68020 1 clock); at -O2 weight by block frequency (the coalescer's `copy_frequency` exists).
- Other 6 targets: none (m68k crate, option off in `Options::PINNED`); no shared-crate edit; md5 identity unaffected by construction.
- Compile cost: linear web build; the per-function trial doubles a function build only where the pass fired (as #10797).
- A real live-range class splitter in greedy would be the pin's `splitAroundRegion` with class change: not in the pin, so a DEVIATE with no counterpart: not recommended.

## (d) Estimate, milestone, gate
- 4-6 days. First milestone (day 2): web class choice on the `g()` shape and `blok_pair`; fail-first test: a phi pointer web with address uses and one data use ends with 0 class crossings; QB over-50 A/B.
- Realistic yield 1.0-1.5 KB over 123 (cross-block 0.7, loads 0.25, part of adds/shifts), because the largest piece (adds, 2.2 KB) is half done by #10797 and d->d (the largest copy class) is out of reach. Under the 3 KB bar: do not build. Propose the next cell instead.

## (3) The non-class classes (moveq +2,972 B, ALU +3,590 B, over-50)
- They are mostly interchange-equal-size forms, not extra work: `moveq #0,%d` ours 2365 vs gcc 124 against `clr` ours 1869 vs 3868 (2 B each; together ours 4234 vs 3992 insns, ~0.5 KB). `moveq #k` + register ALU is 4 B against `addil/andil #k` 6 B (ours smaller); gcc spends the gap on `pea`/`lea`/`adda #n,%sp` stack forms (-1.7, -1.1 KB) and register shifts (`moveq`+`lsl` ours 114 vs gcc 463).
- ALU memory-source forms: ours 1203 vs 1389, memory-destination RMW 88 vs 176 (~+0.2 KB). Nothing above ~0.5 KB; no pass-level class in these mnemonics.
- Hence after copies (4.1 KB upper bound) the other 2.3 KB of the +6,404 B is diffuse.

## Next cell (per the rule: ceiling under 3 KB)
1. m68k per-function hunk reading of `lexr_line` (+660 B) and `decl_line` (+1,458 B, gcc keeps `decl_field` 1,168 B out of line): names the pass-level cause per hunk (coordinator item 3); no builds.
2. arm64 uncovered rows: 21 modules, +~600 B diffuse (row ARM64-OS-QB-RESIDUE): not worth a cell.
3. m32/m64 `decl_field` class: pin llc larger than ours, RA class, no pin pass (earlier finding): not worth a cell.
