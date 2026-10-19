---
architecture-index: JumpThreading ON loses on 13 rows: cause and owner per row
architecture-order: 278
---
# Rows where JumpThreading ON is slower than OFF

Bench pair on `846c0c0c6` (one binary, plain vs JumpThreading on, [`jumpthreading-translation.md`](jumpthreading-translation.md)): clocks geomean llrm/min(gcc,clang) O2 .9149 to .9161, O3 .8851 to .8889, Os .9280 to .9333; bytes smaller everywhere. 13 rows are worse, 16 better. JumpThreading's output equals `opt -passes=jump-threading` on every input compared, so the losses sit downstream of the pass. Direction: land and fix forward, no switch-off; each row has an owner and a repro.

Every number below was re-measured on `5334cc1dc` with the emulator rows harness (A/A identical) and matches bench. "Off" is the default; "on" is `LLRM_JUMPTHREADING=1`.

| Row | Clocks off to on | Cause | Owner | Repro and expected |
|---|---|---|---|---|
| queens -O3 | 96949 to 123141 (+27.0%); ins 78977 to 106279; mem ops 12911 to 25877 | ipa-cp stops cloning `_place` at the 6th clone: its call-site benefit does not compound ×10 per level because the JumpThreading-shaped bodies make the call sites cold (`ipacp.rs`). Our block frequencies equal `opt`'s `print<block-freq>` on the same IR, JumpThreading on and off (stage 11 and the stage-12 clones), so the estimator is not at fault. Forcing every clone gives 93729 clocks | cycles-queue-quiet-host (ipa-cp clone verdict; exit W2 `decide`) | `LLRM_DEBUG=ipa-cp llrm-c -O3 queens.c`, `_place` lines. Off: benefit 120, 2480, 9112, 84387, 813614, 7878001, then 447, 4779 (size 26, 8 clones in all). On: 108, 3031, 3494, 1594, 120, 2 (size 24, `kept` at the 6th, 5 clones). Expected: the off sequence |
| recpow m16 -O2 (compile) | 89.26 M to 105.41 M instructions (+18.1%); output grows | Old clocks inliner (`ipa-recursive.`, `ipa-peel.`) inlines a larger body when the callee shrinks; the pass itself is about 1 M of the 5.35 M extra work. No counterpart at the pin | cycles-queue-quiet-host | `perf stat -e instructions:u llrm-c bench/recpow/recpow.c -O2 -march=i486` on/off. Exit: W2 `inlinecost::decide`. DEVIATE J12 |
| hanoi -O2 | 109 to 114 (+4.6%); ins 67 to 81 | Entry does `push ecx; mov ecx,eax` before the `n==0` test; the early-return path pays the frame. Needs shrink-wrapping (`shrink-wrap` is a Missing row) | backend-two (B7 `prologue@13`: ShrinkWrap/CSR save placement and the argument register convention, [hanoi -Os](#hanoi--os-llc-comparison)); `xor eax,eax` before the test: lir-cleanup-port (`MachineSink`). conflict: needs check for `mov ecx,eax` (see below) | `llrm-c -O2 -S bench/hanoi/hanoi.c` on/off: `diff` shows the `push ecx` and the copy. Expected: no frame on the `n==0` path |
| hanoi -Os (not in the 13) | 188 to 298 (+58.5%); ins 98 to 166 | Same as -O2: `push ecx; mov ecx,eax; xor eax,eax` before the test, loop copies `mov eax,ecx` | backend-two (ShrinkWrap/CSR save placement, argument convention); `MachineSink` (lir-cleanup-port) for the constant. jumps-tailmerge-2241 owes no change | same command with `-Os`; expected 188 clocks |
| x_switch_sparse -O2/-O3 | 129822 to 134658 (+3.7%); ins 63329 to 65341 | JumpThreading (as the pin) removes the empty default block, so the default phi value reaches the join from the switch block. Our backend then puts `mov edx,1` in each compare-tree leaf, before its `cmp`, and the case paths execute it for nothing. The pin's `llc` keeps the default copy in its own block (`mov edi,1; jmp join`) | lir-cleanup-port (translate pre-RA `MachineSink`, [Shared cause](#shared-cause-the-phi-constant-is-not-sunk)) | `llrm-c -O2 -S x_switch_sparse.c` on/off: `mov edx, 1` before `cmp eax, 90`, `1000`, `30000`. Expected: one `mov edx,1` in a default block (as `clang -O2 -march=i486`) |
| x_switch_dense -O2/-O3 | 117135 to 121132 (+3.4%); -O3 115431 to 121132 (+4.9%); ins 53716 to 57115 | Same mechanism: `mov eax,1` hoisted into the leaf before `cmp edx,11; ja`; the loop tail reads `xor eax,eax; mov al,[table]; jmp` where off has `movzx; add; inc; jne` (first read as lost tail duplication; `llc -O3` on our IR keeps the same shared latch, `movl $1,%edi; cmpw $11,%si; ja .LBB0_4` then `.LBB0_4: addl %edi,%eax; decl %ecx; je exit`, so the shape is the JumpThreading-on IR's, not placement) | -O2 `mov eax,1` before `cmp; ja`: lir-cleanup-port (`MachineSink`, by asm reading). -O3: lir-cleanup-port isel row (zero-extending byte load, see below); tail duplication is not the cause (`llc` keeps the shared latch too). conflict: needs check for the -O3 `mov eax,1` | `-O3 -S x_switch_dense.c` on/off. Expected: `movzx` for the table load, as `llc` emits `movzbl` |
| x_switch_ranges -O2/-O3 | 61433 to 63242 (+2.9%); -O3 59629 to 63242 (+6.1%) | Same family as dense: extra `push esi` and a `jmp` to the shared tail instead of a duplicated tail with `inc ecx; jne` | lir-cleanup-port (`MachineSink`, by asm reading). conflict: needs check for the shared tail: `llc` keeps the shared latch on dense -O3, so the duplicated tail may not be expected here; `llc` on ranges not run | `-O3 -S x_switch_ranges.c` on/off. Expected: to be fixed after `llc` is run on this IR |
| x_switch_sparse -Os (not in the 13) | 187802 to 193757 (+3.2%) | As sparse -O2 | lir-cleanup-port (`MachineSink`) | `-Os` |
| mandel -O2/-O3 | 646935 to 671370 (+3.8%); ins 168535 to 184145 (+9.3%); code 179 to 163 B | The inner loop's break exit now feeds the `iteration` phi directly. Copies `mov bx,32` and `mov ebx,eax` run inside the loop on every iteration; off, they sat in exit blocks outside it (`L0_38`, `L0_41`) | lir-cleanup-port (`MachineSink`); back to greedy-inline-spiller if the copies still coalesce into the loop afterwards | `llrm-c -O2 -S bench/mandel/mandel.c` on/off. Expected: the phi copy on the exit edge, outside the loop |
| nbody -O2 | 366772 to 372570 (+1.6%); mem ops 20043 to 23243 | In the inner loop two x87 loads are stored to stack temporaries `[esp+12]`, `[esp+4]` and reloaded each iteration (off: kept on the FP stack); frame 148 to 160 B | backend-slopes (B3 `floatassign@7`; `floatalloc@8` is Translated) | `-O2 -S bench/nbody/nbody.c` on/off, inner loop at `L0_60`. Expected: no `fstp [esp+4]` in the loop |
| fpbench -O2 | 151463 to 151631 (+0.1%); ins -0.6% | Noise-level: instructions fall, clocks rise by 168 of 151 K. Not investigated | none | none |
| x_cordic -O3 | 27616 to 27624 (+0.03%); ins +4 | Noise-level: 8 clocks of 27.6 K. Not investigated | none | none |

## Not in the 13, seen in the same pair

- Code size worse with JumpThreading on: queens -O2 +8.7%, sieve -O3 +13.6%, x_switch -Os +6.6%, x_switch_sparse -O2/-O3/-Os +4 to 6%. Bytes are better in total (m16 -184/-286/-401, m32 -470/-890/-152 at O2/O3/Os). Same family as the switch rows; no separate cause was looked for.
- Instructions up with clocks better: bintree -O3 +7.3% ins, -4.3% clocks.
- Wrong output at m32 -Os on both sides, not from JumpThreading: recgcd, switch.

## Shared cause: the phi constant is not sunk

hanoi, the three switch rows and mandel share one cause. In `llc` the pre-RA `MachineSink` (`machine-sink`, before `PHIElimination`) splits the critical edge and sinks the phi's constant definition (`MOV32r0` or `MOV32ri`, whose only user is the PHI) into the new block. `PHIElimination` then puts its COPY at the end of that block. On hanoi, `-stop-before=machine-sink` has the constant in `bb.0` before the test, as we do. `-stop-after=machine-sink` has a new `bb.3` holding it. `llc` gives the same code for the on and off IR: `xorl %eax,%eax; retl` in its own block.

We have no `MachineSink`; `PressureSink` moves loads only. It is not `PHIElimination`, two-address or isel (`LLRM_NO_PHIELIM_LLVM` changes nothing). Owner: lir-cleanup-port, translating `MachineSink`; first PR planned 2026-10-22, fail-first test the hanoi body, rows pair by bench. Evidence for `MachineSink` is the hanoi `llc` run above; for the switch rows and mandel it is a reading of our assembly, `llc` was not shown on them. The frame setup and the argument register belong to backend-two.

## hanoi -Os: llc comparison

jumps-tailmerge-2241 ran `llc -O3`/`-Os` on our IR (same dump). `llc` emits no `push`/`pop` for hanoi: the argument stays in `ecx` (fastcc-like). Ours moves it to a callee-saved `ecx` after `push ecx`, so the join block is `pop ecx; ret`: 2 instructions against the `-Os` tail duplication size of 1, so the `ret` is not duplicated. Cause: ShrinkWrap/CSR save placement and our argument register convention (backend-two). Tail duplication would apply once the save is on the call path only. jumps-tailmerge-2241 owes no change on hanoi or x_switch_dense -O3.

Conflict, needs check: the first reading above calls the `mov ecx,eax` before the test a `MachineSink` copy. `llc -Os` shows the argument kept in `ecx`, so `mov ecx,eax` looks like the argument move (convention), and only `xor eax,eax` is the phi constant. Which instruction `MachineSink` moves on hanoi is to be confirmed by lir-cleanup-port against the `llc` MIR.

## x_switch_dense -O3: isel

The one machine difference against `llc` on this IR is isel: ours `xor eax,eax; mov al,[table]`, `llc` `movzbl`. Owner: lir-cleanup-port, isel row (zero-extending byte load). The IR that JumpThreading-on produces differs from off in the shape the loop sees; the IR diff was not read (`d.x_switch_dense.O3` in the scratch dumps is empty).

## Open

- Fail-first tests: the owner writes them against the repro. A reduced IR fixture exists only for the switch shape: the `pipeline.body` of `x_switch_sparse` entering `jumpthreading@668` (`switch` with an empty default block and an 8-way phi). `JumpThreading` output equals `opt`; the assertion belongs in the backend: no `mov` of the default value before the first `cmp` of a leaf.
