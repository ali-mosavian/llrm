# Compile-time scaling

For maintainers asking how the compile cost of llrm, gcc and clang grows with program size, and which llrm step grows fastest. This file states the method; `scaling.py report` prints the numbers. The comparison family is in [vs-gcc-clang](../../../../docs/comparison/vs-gcc-clang.md#compile-time).

    uv run --project tools --with matplotlib python crates/target/llrm-x86-m32/vsgcc/scaling.py run [--axis NAME ...] [--programs] [--min 4] [--max 15] [--runs 3] [--limit 10] [--jobs 6]
    uv run --project tools --with matplotlib python crates/target/llrm-x86-m32/vsgcc/scaling.py report
    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/scaling.py check [--n 64]
    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/scaling_gate.py [--jobs N]

## Flow

```
 AXES[axis](N)  N = 2^min .. 2^max, doubling       QCport modules (--programs)
        |                                                   |
        v  one C program per (axis, N)                      v
 +--------------+--------------+--------------+     one point per module:
 | llrm-c -m32  | gcc -m32 -c  | clang -m32 -c|     (MIR instructions, cost)
 |  levels_time.command, -O0 -O1 -O2 -O3 -Os   |     MIR size: LLRM_DEBUG=mir
 +------+-------+------+-------+------+-------+
        |  perf stat -e instructions:u,task-clock, median of --runs
        |  (llrm -O2 also: LLRM_DEBUG=time, own cost per step)
        v
 minus the same compiler's cost on an empty file
        |
        v
 $VSGCC_WORK/scaling.json --report--> tables + one PNG per axis
 an axis stops for a compiler once one compile takes --limit seconds
```

## What is measured

Cost is `instructions:u` (user instructions retired, children included) and `task-clock`, less what the same compiler spends on an empty file. The slope is the exponent of cost against N over the sizes reached, and over the largest four. A slope of 1 is linear. The compilers run through `levels_time.py`, so the commands are the compile-time comparison's.

`report` prints, per axis and level: llrm's slope, gcc's and clang's slopes at the same N and over their own range, the llrm/gcc ratio at the smallest and largest N (total and net of the empty file), and the number of sizes each reached. For llrm `-O2` it also prints the steps whose slope exceeds 1.15 with their own cost at the largest N (own cost excludes children; it is `LLRM_DEBUG=time`).

## Axes

Each axis is a generated C program that prints a checksum; `check` compiles every axis at one size with gcc and clang on the host (`-O0`, `-O2`) and with llrm `-O2` in the emulator, and requires one value, so an axis cannot read a compiler that miscompiled it.

| Axis | Program of size N | Stresses |
|---|---|---|
| `functions` | N small functions, a loop and a call each | module-level analyses, per-function fixed cost |
| `straight` | one function of N statements on four live values | coalescing, two-address, allocation |
| `mulconst` | N multiplies by large odd 32-bit constants | instruction selection's multiply search |
| `branches` | one function of N branches and loops over six live values | allocation under pressure |
| `live` | N volatile loads live across a call, then all used | spilling |
| `callers` | N callers of eight small callees | the inliner |
| `chain` | a call chain N deep, one caller per link | interprocedural summaries |
| `nest` | loops N deep, a statement at each depth | passes that walk a loop's nest |
| `cases` | a loop around a `switch` of N cases | a block with N successors |
| `unrolled` | a recursive partition unrolled to N loops in one function | analyses that meet each loop once per loop |
| `cells` | N distinct memory cells read after the stores before them | memory forwarding |
| `joins` | N held cells and N if/else joins that store and read | through-memory's dense solve |
| `fjoins` | `joins` in floats | float facts |
| `fanout` | one function using one value N times | passes that walk a value's users once per use |
| `equal` | one function of N equal expressions | value numbering's replacement of N values by one leader |

## QCport

`--programs` adds QCport's 65 modules: one point each of (llrm's MIR instructions, cost), so the slope is cost against code size rather than a generated shape. Needs `QCPORT` and `QCPORT_INC` (`--qcport`, `--inc`). QCport is Open Watcom DOS C, which gcc and clang do not read as it stands, so a copy is stubbed for all three (llrm reads the same stubbed text): 16-bit struct-size asserts, inline assembly, a ^Z end-of-file byte in Borland headers, and Borland names (`_fmemcpy`, `stricmp`, `FP_OFF`, ...) declared in a prelude for gcc and clang only, with `-Dfar= -Dhuge= -Dnear= -Dpascal= -Dcdecl=`. A module a compiler fails on is listed with the reason in the report.

## Gate

`scaling_gate.py` is the gated form, run by `tools/measure.py`: it takes llrm-c's cost at N/2, N and 2N for the axes in its `SIZES` (and `chain` and `callers` again at `-m16`), less the empty file's, and forms `c(2N) - 3c(N) + 2c(N/2)`, which is nil for fixed and linear work and `1.5kN^2` for a quadratic term. It reads the same for each step with at least a `LOW` share of the work. It exits 77 where `perf` cannot count. Tolerances: `tools/gate/tiers.toml` `[measure]`. Method: [compile-time](../../../../docs/working/measurement/compile-time.md).

## Known remaining superlinear

What still grows faster than its input at `-O2`, by axis and step (names as `LLRM_DEBUG=time` prints them). Read the numbers with
`scaling.py run --axis NAME` and `report`, or `scaling_gate.py`; none are copied here, they would be stale by the next merge.

- **The allocator's spill, facts and interval steps** (#1229, owner: slopes and regalloc). They grow faster than the function does, and
  move together, so a fix to one row alone will not show. The rows:
  - `branches`, 512 to 1024: `siblings interference`, `ssa simulated`, `spill short updates`, `spill local updates`, `intervals homes`,
    `lir phielim`, and `regalloc` `spill`, `candidates` and `after queue`.
  - `chain` and `live`, 128 to 256: the same spill-round rows.
- Rows that are superlinear on one axis and flat on the others belong to a pass, not to this cause (`mir unroll` on `branches` is one).
  They have their own issues.

To tell the two apart, take the axis where only the spill rounds change (`live`, `branches`): rows that move together there are #1229's.

## Caveats

- Instruction counts are not time; `task-clock` is, and it is host dependent.
- Slopes are over the sizes a compiler reached. A compiler stops once one compile takes `--limit` seconds, so the compilers' ranges differ: read the range columns.
- A step's slope at small N is dominated by fixed cost; one whose ratio is a small-N artifact is recorded as it reads.
- The measured command runs with a fixed environment size (`measured_environment`): the bytes of the caller's environment move the stack and allocator and change a count.
- gcc's count includes its assembler and clang's an integrated one; llrm writes the object directly.
- A generated program is a shape, not a program people write. A slope on an axis says that a pass is superlinear in that shape, not that real code hits it.
- The `-m32` axes run only at `-m32`; `-m16` is covered by the gate's two extra axes.

## Test

    uv run --project tools --group dev pytest crates/target/llrm-x86-m32/vsgcc/test_scaling.py crates/target/llrm-x86-m32/vsgcc/test_scaling_gate.py

`test_scaling.py` runs a stand-in compiler of known cost: a quadratic one must read as slope 2, a linear one as 1, and the generators must not collapse.
