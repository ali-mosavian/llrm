# Current target coverage

Measured **2026-09-10**, compiler revision **8e951b5**, with
`uv run python tools/opportunity.py --targets` over all **487 objects** in
`fixtures/omf` (**34 source programs**). These are model-weighted instruction
costs, including configured helper costs—not hardware timings. Default loop
weighting is ten iterations per nesting level, capped at three levels.
All rows were rescored in one integration run after the recent raise, loop
and lowering changes. [Raw results](target-scoreboard-current.txt) retain
every configuration, including provisional and missing references.

| Status | Configurations |
|---|---:|
| Comparable and within 1.5x | 301 |
| Comparable and above 1.5x | 0 |
| Provisional reference | 105 |
| No target | 81 |
| Unmeasured / refused emission | 0 |

**The goal is not complete.** The gate remains failing. Only 301/487 rows currently
have comparable references; this is coverage, not a project-completion
percentage. The architecture checklist remains independently binding.
Comparability here is the scorer's classification, not a new independent
audit of every registered reference. The 105 provisional rows comprise
81 event builds, 12 ordinary FPCSEX builds and 12 ordinary FPDEEP builds.
Recent regression fixtures in `fixtures/regressions` are outside this
default scan and are not implied covered by these totals.

## Largest comparable gap

IVCHAN is worst at **1.35x**: all ordinary variants cost 756 against 560
(QB and VBDOS plain previously cost 762). NESTED reaches
1022/768 (**1.33x**). No comparable row exceeds the requested threshold.

## Floating-point cases

| Program | PDS cost | QB cost | VBDOS cost | Reference status |
|---|---:|---:|---:|---|
| FPCSE | 157 | 145 | 157 | Complete; all three are 1.00x |
| FPCSEX | 4462 | 4467 | 4452 | Provisional: 1340 reassociates additions and omits SINGLE rounding |
| FPDEEP | 1777 | 1652 | 1777 | Provisional: 1086 lacks a complete checkpoint/store observability proof |

These rows use `p-g2`, `q-O`, and `v-g3`. Do not divide by the provisional
numbers to claim success or justify relaxing floating-point behavior.

**Follow-up fix:** QB FPDEEP rose from 1572 to 1652 when upper-word
normalization enabled CSE of PRINT addresses. Lowering now rematerializes
those constants at pushes instead of spilling them across calls: **1652 →
1570**, with all three spill slots removed. The table and raw file retain
the integration baseline above; this is a focused follow-up, not another
487-row scan. PDS/VBDOS FPDEEP and all three FPCSE costs are unchanged.
Both programs pass actual execution on all three compilers. FPDEEP remains
provisional because its reference proof is still incomplete.

One address, before and after (the zero operand receives the same relocation):

```asm
; before                         ; after
mov ax,0 ; seg:9+0x10             push 0 ; seg:9+0x10
mov [bp-2],ax                    call B$PSSD
mov ax,[bp-2]
push ax
call B$PSSD
; later: reload slot and push    ; later: push the same immediate address
```

The fix stays in instruction selection. MIR still shares one symbolic value;
other readers and observable exit values retain their definitions. Full stage
dumps locate the change at lowering, not in any MIR optimization pass.

## Constant-condition follow-up

Follow-up constant-condition folding also reduces QB BOOLS from **164 to 156**
(target 126; **1.30x → 1.24x**), matching PDS/VBDOS. Its constant AND and two
jumps disappear; the object shrinks from 782 to 772 bytes. BOOLS and IVCHAN
execute correctly on all three compilers. These focused results do not replace
the integration snapshot.

```asm
; before                 ; after
mov ax,0FFFFh            add word [t],2
and ax,0FFFFh
jne taken
jmp done
taken: add word [t],2
done:
```

`[t]` names the relocated `seg:5+0x10` operand in the emitted listing.

The next memory-folding change reduces all three ordinary BOOLS variants from
**156 to 128** (**1.02x** against 126). Exact scalar memory updates now carry
constant facts through the store; folding retains a constant store when its
condition results are unused. QB's object shrinks **772 → 749 bytes**.

```asm
; before                     ; after
mov word [t],0               mov word [t],0
add word [t],0FFFFh           mov word [t],2
inc word [t]
add word [t],2
; after printing the label   ; after printing the label
push word [t]                push 2
call B$PEI2                  call B$PEI2
```

The remaining overwritten zero store is visible in the emitted bytes even
though the scorer reports zero recognized redundancy. Neither that zero nor
the near-target ratio proves ideal code. BOOLS, HOTLOP, IVCHAN and PRESSX pass
execution on QB/PDS/VBDOS; live-condition and loop-counter guards have focused
tests. The full integration snapshot above has not been rerun.

The dead-store follow-up removes that zero store by retaining an empty byte-
ownership marker when a block has no surviving neighbor. BOOLS falls again:
**128 → 122** on all three compilers; QB object **749 → 736 bytes**.

```asm
; before                 ; after
mov word [t],0           mov word [t],2
mov word [t],2
```

The measurement blind spot is identified, not repaired: re-raising merges the
preceding word stores into a dword, but the scorer compares starting addresses
and misses the later high-word overwrite. The source-derived BOOLS reference
also needs review: 122 beats its 126, and the final adjacent x/t stores may
still be combinable. Do not describe 126 as a proved minimum.

Three direct `floatloop.specialized` tests in `test_float_loop_exit.py` fail
on the unchanged pre-marker baseline too (QB/PDS/VBDOS). They remain open;
their original expectations were not weakened. Full-pipeline FPCSE execution
is checked separately from those pass-local expectations.

Those pass-local failures are now repaired: explicit `FCHECK` nodes were
missing from the exact-repetition evaluator's supported operations. Pure checks
are accepted numerically and retained, in order, by final-iteration
specialization. Checks with other effects remain unsupported. The direct test
still requires the 438.75 pre-final seed and the complete checked final
iteration. The emission test now checks the final 487.5 store, its relocation,
and PRINT argument instead of requiring an intermediate seed that later folding
eliminates. All 16 focused cases pass; FPCSE/FPDEEP execute correctly on all
three compilers. Full-pipeline FPCSE QB assembly is unchanged by this repair.

The scorer follow-up repairs that partial-overwrite blind spot. Re-measuring
compiler `98943e1` BOOLS QB still gives cost **128**, but now reports **one
partial overwrite of unread stored bytes**, previously zero. Access facts carry
width and address identity; overlapping reads invalidate unread-store evidence,
and indexed addresses cannot establish a fixed-cell redundancy. Partial writes
discard the old whole-cell fact conservatively rather than claiming its remaining
bytes dead. Full-overwrite counts identify prior stores, so one dword overwrite
can account for two prior word stores.

This is measurement-only: before/after assembly is identical. The full 487-row
snapshot has not been rerun; its redundancy counts use the older instrument.

## Missing references

| Source program | Configurations without targets |
|---|---:|
| CHAIN | 15 |
| DIVMOD | 15 |
| FPEMU | 15 |
| JUMPS | 15 |
| PROCS | 16 |
| CM | 4 |
| JT | 1 |

Names come from object source headers, not filename prefixes. Event-enabled
configurations with existing plain targets also need event-preserving
references; their plain-program denominator is not comparable.

## Next work

1. Derive and validate strict FPCSEX and complete FPDEEP reference listings;
   retain source-order rounding, pending exceptions and observable stores.
2. Add independently derived references for the seven uncovered programs
   and event-enabled configurations. Do not scale targets from emitted costs.
3. Use those validated gaps to prioritize implementation alongside the
   [architecture checklist](architecture.md#high-impact-mir-passes).
   Runtime-sized array extents, precise call effects, remaining GVN-PRE,
   loop transforms and backend work are not declared done by this scan.

The integration snapshot itself changes no emitted assembly; the follow-up
compiler change is shown above. Earlier measurements and detailed
investigations remain in [the historical record](target-coverage-history.md).
