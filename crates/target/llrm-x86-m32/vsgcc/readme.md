# crates/target/llrm-x86-m32/vsgcc

For maintainers running llrm's `m32` output against gcc and clang. Method, commands, flow and caveats: [vs-gcc-clang](../../../../docs/comparison/vs-gcc-clang.md). This file lists the files and how to run them.

    crates/target/llrm-x86-m32/vsgcc/run.sh                                              # build, run, print the tables; products in $VSGCC_WORK
    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/harness.py sieve gccO2     # one program, one variant
    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/loops.py sieve llrm gccO2 clangO2   # its hottest loop, side by side
    uv run --project tools --group dev pytest crates/target/llrm-x86-m32/vsgcc                # after run.sh

| File | Does |
|---|---|
| `run.sh` | builds llrm-c and every program with every compiler, runs all, prints `table.py`, writes `loops.PROG.txt` and `ctime.json` |
| `build.sh PROG` | one program at -O1, -O2, -O3, -Os in llrm (OMF; ELF for `loops.py`), gcc and clang |
| `wrap.py` | the copy every compiler builds: `main` calls the kernel through a volatile pointer |
| `harness.py` | links, runs `main` in unicorn, counts the kernel's instructions, memory operands, 486 clocks |
| `table.py`, `loops.py` | the tables, the hottest loop |
| `programs.py`, `kernels/x_*` | the programs; `x_*` are kernels written for this comparison, each with a `.out` self-check from gcc `-O0` on the host, which every variant must reproduce or `table.py` refuses |
| `levels_time.py`, `ctime.py` | compile cost per level, compile wall clock |
| `scaling.py`, `scaling_gate.py` | compile cost against program size: [scaling](scaling.md) |
| `stub.s` | `report`, `memset`, `memcpy`, `memmove` for every variant |

A rerun with another llrm shows a change in the table's llrm columns; gcc and clang do not move.

```
 run.sh -> build.sh -> wrap.py -> {llrm-c, gcc, clang} -> harness.py (unicorn)
                                                     -> results.jsonl -> table.py
                                          loops.py <-- ELF of llrm, gcc, clang
```
