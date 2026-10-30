# tools/pairs

Pair measurements: two builds of the compiler, the same rows, objects and runs on both, one report.

- `pair_build.sh`, `buildseq.sh`: one dist build per sha, cached.
- `rowdiff.py`, `objsweep.py`, `rundiff.py`, `runpinned.sh`: the rows, the objects and the tests/run outputs of the two arms. `runpinned.sh` runs tests/run from a tree checked out at the binaries' sha.
- `board.py`, `cbisect.py`, `firstbad.py`: the d_faces compile board, the compile-cost bisect, the first bad commit of a failure.
- `voidpair.py`: refuses a pair whose premise is broken.
- `pincount.py`: static instructions of llrm-c against the pin's `llc` on the same (our) IR, per program; totals and the programs furthest above the pin. The gap is the backend's. `--env LLRM_SRC_ORDER=1` for a flag.
- `pindiff.py` (corpus: `--src DIR`, default the repo's `vsgcc/programs.py` programs copied flat, written with u32/i32 so m16 shifts stay in range): ours against the pin's `llc` on one IR (clang with llrm-cc's flags, then the pin's `opt`), per program: instructions, code bytes and code bytes without nop padding (the flagged column), m16 or m32, O1/O2/O3/Os; `irguard.py` refuses an IR whose `target-cpu` is not the CPU both llcs run with, `omfcode.py` counts an OMF object's code bytes without its data. Header names the llc's md5 and the pin.
- `pindiff.py ... --dyn` (m32, needs iced_x86 + unicorn on `PYTHONPATH`): also runs each kernel in the vsgcc harness (`dyn.py`, kernel called through a volatile pointer) and adds executed 486 clocks of ours and the pin's; rows that run slower (`CLK>pin`) or print another value (`OUTPUT-DIFFERS`) are flagged and listed first. Static counts flattered `tile` (-79 ins, +94.6% clocks).
- Same-IR check (`llrm-c ... --dump DIR` on disk, then `pin.py llc -m 32 DIR/<stage>.ll`, `-frame-pointer=none`): a stage after `27-lsr` is valid for register allocation and instruction selection only; the pin's llc runs LSR again on it, so a loop-strength difference there is not one (x_transpose's last loop looked 2 instructions worse and was identical). Take `26-canonfreeze` for LSR.
- Judging a PR with `pindiff`: build the PR merged into current `release/next` and `release/next` alone, and blame the PR only for what differs between the two. A PR's own head can be hundreds of commits behind (#5402 was 457): its llc gave matmul -O3 a wrong checksum that its base had and parity does not.
- `crashsweep.py LLC WORK`: every corpus program at O0..Os x m16/m32 x dag/old through ours; exits 1 on a compile failure outside its KNOWN list (each entry names its owner) or a KNOWN entry that no longer fails.
- `rowdelta.py BEFORE AFTER`: the rows that moved between two pindiff tables of the same corpus and level (an A/B of two builds).
- `drift.py SHA_A SHA_B` (or `--tables BEFORE AFTER`): the pindiff rows that grew between two parity commits, per machine and level (m32/m16 x O1/O2/Os), the most instructions first; exit 1 when any grew. Uses each commit's `pair_build` dist `llrm-llc` and this checkout's pindiff.
- Known m16 bias: rows with i32 arguments or returns differ by ABI, not codegen. Ours (cdecl) returns i32 in dx:ax and pushes 32-bit args; the pin's code16 returns in eax.
- Known m16 bias: the pin's llc replaces the module datalayout with its triple's (`p:32:32`), so its ISel/LSR/CGP see 32-bit pointers; its opt and ours see `p:16`. pindiff prints a NOTE line.
- `pin.py`: the one way to run the pin's `opt` or `llc` on llrm's IR: it takes an explicit `Target` (triple, CPU, datalayout from the target description, features, `optsize`), writes it into the module, refuses without one and prints a `target:` line. A module that went to the pin without it was priced for x86-64 with SSE; such numbers are void.

A binary's md5 is not evidence that two builds are the same: embedded paths differ between worktrees. A claim that a change "cannot move" anything is made from the compared OBJECTS (`objsweep.py`); the md5 only proves that two arms are different binaries.

Quote every figure with its tool (callgrind Ir, `perf stat -e instructions:u`, a rows counter) and the arms line of the report.
