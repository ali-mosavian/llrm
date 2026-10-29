---
architecture-index: JumpThreading ON loses on 13 rows: cause and owner per row
architecture-order: 278
---
# Rows where JumpThreading ON is slower than OFF

Bench pair on `846c0c0c6` (one binary, plain vs JumpThreading on, [`jumpthreading-translation.md`](jumpthreading-translation.md)): clocks geomean llrm/min(gcc,clang) O2 .9149 to .9161, O3 .8851 to .8889, Os .9280 to .9333; bytes smaller everywhere. 13 rows are worse, 16 better. JumpThreading's output equals `opt -passes=jump-threading` on every input compared, so the losses sit downstream of the pass. Direction: land and fix forward, no switch-off; each row has an owner and a repro.

Every number below was re-measured on `5334cc1dc` with the emulator rows harness (A/A identical) and matches bench. "On" is the default; "off" is `LLRM_NO_JUMPTHREADING` set.

| Row | Clocks off to on | Cause | Owner | Repro and expected |
|---|---|---|---|---|
| queens -O3 | 96949 to 123141 (+27.0%); ins 78977 to 106279; mem ops 12911 to 25877 | ipa-cp stops cloning `_place` at the 6th clone: its call-site benefit does not compound ×10 per level because the JumpThreading-shaped bodies make the call sites cold (`ipacp.rs`). Our block frequencies equal `opt`'s `print<block-freq>` on the same IR, JumpThreading on and off (stage 11 and the stage-12 clones), so the estimator is not at fault. Forcing every clone gives 93729 clocks | cycles-queue-quiet-host (ipa-cp clone verdict; exit W2 `decide`) | `LLRM_DEBUG=ipa-cp llrm-cc -O3 queens.c`, `_place` lines. Off: benefit 120, 2480, 9112, 84387, 813614, 7878001, then 447, 4779 (size 26, 8 clones in all). On: 108, 3031, 3494, 1594, 120, 2 (size 24, `kept` at the 6th, 5 clones). Expected: the off sequence |
| recpow m16 -O2 (compile) | 89.26 M to 105.41 M instructions (+18.1%); output grows | Old clocks inliner (`ipa-recursive.`, `ipa-peel.`) inlines a larger body when the callee shrinks; the pass itself is about 1 M of the 5.35 M extra work. No counterpart at the pin | cycles-queue-quiet-host | `perf stat -e instructions:u llrm-cc bench/recpow/recpow.c -O2 -march=i486` on/off. Exit: W2 `inlinecost::decide`. DEVIATE J12 |
| hanoi -O2 | 109 to 114 (+4.6%); ins 67 to 81 | Entry does `push ecx; mov ecx,eax` before the `n==0` test; the early-return path pays the frame. Needs shrink-wrapping (`shrink-wrap` is a Missing row) | domtree-port (B7 `prologue@13`: ShrinkWrap/CSR save placement, per coordinator 2026-10-20; [hanoi -Os](#hanoi--os-llc-comparison)); `xor eax,eax` before the test: lir-cleanup-port (`MachineSink`). conflict: needs check for `mov ecx,eax` (see below) | `llrm-cc -O2 -S bench/hanoi/hanoi.c` on/off: `diff` shows the `push ecx` and the copy. Expected: no frame on the `n==0` path |
| hanoi -Os (not in the 13, not JumpThreading) | 188 to 298 clocks (+58.5%), ins 98 to 166 (earlier bench pair, JT-off baseline). Re-measured on `f929acb44` (after ShrinkWrap B7, #3067/#3078): JT on and off both 298 clocks, 166 ins, byte-identical `-S`, `_hanoi` 26 B, object 246 B. Not closed. The 188 baseline was probably another build (inference) | ShrinkWrap B7 landed (domtree-port, #3067/#3078) but `_hanoi` still has `push ecx` at entry and `pop ecx` at `L0_2`. The entry block writes `ecx` (`mov ecx,eax`), so the save cannot move off the `n==0` path. That copy exists because the phi constant `xor eax,eax` still sits before `test ecx,ecx; je` and clobbers `eax`. The sunk form (`xor eax,eax` in the ret block, no `push`/`pop`) reported for domtree-port does not reproduce on this head with `-m32 -mabi=sysv -Os -march=i486 -S` | lir-cleanup-port: `MachineSink` for the phi constant (see below); the frame goes with it. Row owner: parity-pipeline | `llrm-cc -m32 -mabi=sysv -Os -S bench/hanoi/hanoi.c`, rows tool (wrapped hanoi, llrmOs). Exit: `_hanoi` has no `push`/`pop` and measures about 188 clocks |
| x_switch_sparse -O2/-O3 | 129822 to 134658 (+3.7%); ins 63329 to 65341 | cause unknown. Seen: JumpThreading (as the pin) removes the empty default block, so the default phi value reaches the join from the switch block, and our compare-tree leaves run `mov edx,1` before their `cmp`. `llc` on our IR also has the default value before the compare, so this is not the difference; see [x_switch and mandel](#x_switch-and-mandel-cause-unknown) | lir-cleanup-port (open question, verdict 2026-10-26) | `llrm-cc -O2 -S x_switch_sparse.c` on/off: `mov edx, 1` before `cmp eax, 90`, `1000`, `30000`. Expected: set by the open question |
| x_switch_dense -O2/-O3 | 117135 to 121132 (+3.4%); -O3 115431 to 121132 (+4.9%); ins 53716 to 57115 | cause unknown. Seen: `mov eax,1` before `cmp edx,11; ja`; at -O3 the loop tail reads `xor eax,eax; mov al,[table]; jmp` where off has `movzx; add; inc; jne`. `llc -O3` on our IR keeps the same shared latch and the default value before the compare, so neither placement nor tail duplication explains it; the `xor`+`mov al` pair is a 486 clock win (ours is better). The loop bodies differ in the multiply and divide, see [x_switch and mandel](#x_switch-and-mandel-cause-unknown) | lir-cleanup-port (open question, verdict 2026-10-26) | `-O3 -S x_switch_dense.c` on/off. Expected: set by the open question |
| x_switch_ranges -O2/-O3 | 61433 to 63242 (+2.9%); -O3 59629 to 63242 (+6.1%) | cause unknown. Seen: an extra `push esi` and a `jmp` to the shared tail instead of a duplicated tail with `inc ecx; jne`. `llc` keeps the shared latch on dense -O3, so a duplicated tail may not be expected here; `llc` on ranges not run. See [x_switch and mandel](#x_switch-and-mandel-cause-unknown) | lir-cleanup-port (open question, verdict 2026-10-26) | `-O3 -S x_switch_ranges.c` on/off. Expected: set by the open question |
| x_switch_sparse -Os (not in the 13) | 187802 to 193757 (+3.2%) | cause unknown; as sparse -O2 | lir-cleanup-port (open question, verdict 2026-10-26) | `-Os` |
| mandel -O2/-O3 | 646935 to 671370 (+3.8%); ins 168535 to 184145 (+9.3%); code 179 to 163 B | cause unknown. Seen: the inner loop's break exit now feeds the `iteration` phi directly, and `mov bx,32` and `mov ebx,eax` run inside the loop on every iteration; off, they sat in exit blocks outside it (`L0_38`, `L0_41`). `llc` was not run on this IR | lir-cleanup-port (open question, verdict 2026-10-26); back to greedy-inline-spiller if the copies still coalesce into the loop afterwards | `llrm-cc -O2 -S bench/mandel/mandel.c` on/off. Expected: set by the open question |
| nbody -O2 | 366772 to 372570 (+1.6%); mem ops 20043 to 23243 | In the inner loop two x87 loads are stored to stack temporaries `[esp+12]`, `[esp+4]` and reloaded each iteration (off: kept on the FP stack); frame 148 to 160 B | backend-slopes (B3 `floatassign@7`; `floatalloc@8` is Translated) | `-O2 -S bench/nbody/nbody.c` on/off, inner loop at `L0_60`. Expected: no `fstp [esp+4]` in the loop |
| fpbench -O2 | 151463 to 151631 (+0.1%); ins -0.6% | Noise-level: instructions fall, clocks rise by 168 of 151 K. Not investigated | none | none |
| x_cordic -O3 | 27616 to 27624 (+0.03%); ins +4 | Noise-level: 8 clocks of 27.6 K. Not investigated | none | none |

## Not in the 13, seen in the same pair

- Code size worse with JumpThreading on: queens -O2 +8.7%, sieve -O3 +13.6%, x_switch -Os +6.6%, x_switch_sparse -O2/-O3/-Os +4 to 6%. Bytes are better in total (JumpThreading on minus off, 71 vsgcc programs, emitted LEDATA, one build with `LLRM_NO_JUMPTHREADING` as the switch, re-taken 2026-10-22 on parity a9a0289b53 with the kernels written for 32 bits: m16 -937/-2621/-53, m32 -672/-2526/+23 at O2/O3/Os; the earlier -184/-286/-401 and -470/-890/-152 came from different builds and are replaced). The per-program list above is not re-taken. Same family as the switch rows; no separate cause was looked for.
- Instructions up with clocks better: bintree -O3 +7.3% ins, -4.3% clocks.
- Wrong output at m32 -Os on both sides, not from JumpThreading: recgcd, switch.

## The phi constant is not sunk (hanoi)

In `llc` the pre-RA `MachineSink` (`machine-sink`, before `PHIElimination`) splits the critical edge and sinks the phi's constant definition (`MOV32r0` or `MOV32ri`, whose only user is the PHI) into the new block. `PHIElimination` then puts its COPY at the end of that block. On hanoi, `-stop-before=machine-sink` has the constant in `bb.0` before the test, as we do. `-stop-after=machine-sink` has a new `bb.3` holding it. `llc` gives the same code for the on and off IR: `xorl %eax,%eax; retl` in its own block.

We have no `MachineSink`; `PressureSink` moves loads only. It is not `PHIElimination`, two-address or isel (`LLRM_NO_PHIELIM_LLVM` changes nothing). Owner: lir-cleanup-port, translating `MachineSink`; first PR planned 2026-10-22, fail-first test the hanoi body, rows pair by bench. The evidence is the hanoi `llc` run above and applies to hanoi only. The frame setup (ShrinkWrap) belongs to domtree-port.

## hanoi -Os: llc comparison

jumps-tailmerge-2241 ran `llc -O3`/`-Os` on our IR (same dump). `llc` emits no `push`/`pop` for hanoi: the argument stays in `ecx` (fastcc-like). Ours moves it to a callee-saved `ecx` after `push ecx`, so the join block is `pop ecx; ret`: 2 instructions against the `-Os` tail duplication size of 1, so the `ret` is not duplicated. Cause: ShrinkWrap/CSR save placement (domtree-port, per coordinator 2026-10-20) and our argument register convention. Tail duplication would apply once the save is on the call path only. jumps-tailmerge-2241 owes no change on hanoi or x_switch_dense -O3.

Conflict, needs check: the first reading above calls the `mov ecx,eax` before the test a `MachineSink` copy. `llc -Os` shows the argument kept in `ecx`, so `mov ecx,eax` looks like the argument move (convention), and only `xor eax,eax` is the phi constant. Which instruction `MachineSink` moves on hanoi is to be confirmed by lir-cleanup-port against the `llc` MIR.

## x_switch and mandel: cause unknown

An earlier reading blamed the phi constant (`MachineSink`), a lost tail duplication and `movzbl`. lir-cleanup-port measured `llc` on our IR and none holds:

- `llc` also has the default value before the compare.
- `llc` keeps the same shared latch on x_switch_dense -O3, so the shape is not placement or tail duplication.
- The `xor eax,eax; mov al,[table]` pair against `llc`'s `movzbl` is a 486 clock win: ours is better.
- The loop bodies differ in the multiply and the divide. Ours is a 14-instruction chain where `llc` has one `imull`, and `div di` by 14 and 10 (24 clocks) where `llc` multiplies by the reciprocal.

Owner of the open question: lir-cleanup-port (`cpu.rs` imul/div pricing), once bench sends per-loop clocks; verdict 2026-10-26. The JumpThreading-on IR diff for x_switch_dense -O3 was not read (`d.x_switch_dense.O3` in the scratch dumps is empty).

## Open

- Fail-first tests: the owner writes them against the repro. A reduced IR fixture exists only for the switch shape: the `pipeline.body` of `x_switch_sparse` entering `jumpthreading@668` (`switch` with an empty default block and an 8-way phi). `JumpThreading` output equals `opt`; the assertion belongs in the backend: no `mov` of the default value before the first `cmp` of a leaf.
