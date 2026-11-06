# m68k timing

Owner: llrm-target-m68k

Clocks of one instruction on each CPU of the 68000 family:
`llrm_m68k::timing::cost_on(cpu, text, bytes, taken)`. A measuring
instrument: the compiler does not call it. `examples/cycles.rs` prices the
kernels per CPU (`CYCLES_CPUS=68000,68030,68060`).

## Where the numbers come from

Every number is read from a CSV transcription of the manual's table, never
typed into the code. A cell test cites the table and page.

| CPU | Manual | Column used |
|---|---|---|
| 68000 | MC68000UM §8, Tables 8-1 to 8-14 | execution time |
| 68010 | MC68000UM §9, Tables 9-1 to 9-19 | non-looped |
| 68020 | M68020UM §8 | cache case |
| 68030 | MC68030UM §11.6 | instruction-cache case: row cell + addressing-mode cell |
| 68040 | MC68040UM §10.4 to 10.6 | busiest stage: `max(<ea> calculate, execute base)` |
| 68060 | M68060UM §10, Tables 10-5 to 10-24 | `clocks(reads/writes)`, plus Table 10-5 |

## How the tables were checked

- 68000: transcribed from the page images with the PDF text layer as a second
  read; the hand-coded model it replaced agreed on every form it priced except
  `cmp.l Dn,Dn` (the table says 6, the model said 8).
- 68030: three independent transcriptions agree on all 530 rows.
- 68060: two independent transcriptions agree on all 1,487 cells.
- 68010: two independent transcriptions agree on all 1,628 cells (headers
  differ in layout only).
- 68040: two independent transcriptions agree on all 2,387 cells. The second
  reading found a row the first lost to a repeated page header: `nop` (8,
  1L + 7). `nop` was priced as an error until then.
- 68020: every table was read twice (the instruction tables 8.2.7 to 8.2.16:
  704 cells and marks, no difference). The address tables and the cache-case
  move matrix differ in three cells of memory-indirect rows only, which are
  not priced. The marks (`*`, `**`, `†`, `‡`, `%`) were checked against the
  legends on pages 8-30, 8-33 and 8-34. The hand-coded model it replaced was
  wrong for `mulu.l`, `clr` to memory, `cmpi`, `dbt` and `move to sr`.

## Limits

1. Pipelined CPUs (68020 to 68060) get one number per instruction, not a
   stream's time. The 68040 and 68060 numbers are a stall-free throughput
   estimate; the 68060's second pipe (pairing) is not modelled. Stalls, cache
   and ATC misses are not in any number.
2. `partial` marks a data-dependent term at its bound: multiply and divide
   (maxima), shift by register (lower bound, 68000/68010), bit fields and
   branches on the 68060 (best case).
3. 68060 branches, `jmp` and `jsr` are priced as predicted correctly; a
   misprediction costs 7.
4. 68060 bit fields are the "< 5 bytes" rows; the "= 5 bytes" rows are longer.
5. 68040: the lead time of a `lead L + base` cell is hidden by the busiest-stage
   rule.
6. Memory-indirect and full-format addressing, floating point, MMU and cache
   instructions, and exception entry (except `trap` on the 68000 and 68010) are
   not priced. An unlisted form is an error, never zero clocks.
7. Loop mode (68010) is not used: an instruction is priced alone.
   68020 bit fields are the "< 5 bytes" rows and `cas`/`cas2` the successful
   compare (`partial`); `movem` to `-(An)` takes the `(An)+` row, as Table
   8.2.4 prints no `-(An)` row.
8. 68010 `MOVEM.L (An),<list>`: the page prints 24+8n, its neighbour 12+8n;
   priced 12+8n. MULS/MULU print two unlabelled rows; MULS takes the first,
   MULU the second.
9. The 68060 "+1 if `(Ay)+`/`-(Ay)` is also the destination `An`" footnote of
   Table 10-9 needs register identity, which the operand reader drops.

## Sources

The manuals' sha256 are in [m68k-isa](m68k-isa.md).
