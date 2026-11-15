# M68K-TRIALS: compile time above 1.5x of the pin at -O2, 68020

Owner: m68k (w8). Exit: 2026-11-24. Renewal needs oversight's ruling.

TL;DR: the four buckets sit at 1.8-3.3x of the pin (`instructions:u`, same unoptimized IR). The gap is backend builds, not the mid-end; trial builds that never strictly win cannot be dropped without a predictor.

## Numbers (2026-11-15, -O2, 68020, release/next f341e06185)
| bucket | ours | pin (opt+llc) | ratio | exit target (1.5x) |
|---|---|---|---|---|
| grep | 271M | 82M | 3.3x | 123M |
| matmul | 712M | 365M | 1.95x | 548M |
| tile | 87M | 48M | 1.8x | 72M |
| particle | 136M | 58M | 2.3x | 87M |

## Why
- matmul: allocate 166M, prealloc 64M, postalloc 48M, expand 30M over ~10 builds of one function; both mid-end runs ~150M.
- Trial winner table, 165 programs at -O2: `shared_cse` strict wins 16 / ties 85; `slots` 10 / 2; relax-on 3 strict wins. A trial may be dropped only if it flips no counted cell.
- The nest trial (second mid-end) is kept in 19 of 27 programs; units and refused-loop count do not separate kept from put back.
- Allocator cost is shared codegen, profiled by 6502-ctime.

## Plan to the exit
- tile, particle: per-pass ratio to the pin, fix passes above 1.5x of the pin's own cost.
- A cell that gets worse gets its own dated row.
