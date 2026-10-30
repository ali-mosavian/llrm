# Codegen ablation

Inputs: the 12 bench C programs (m32, i486) and the 65 QCport modules (m16, code16; 77 IRs the pin parses) from the pin's clang -O0; each through the pin's `default<O2>` (`default<Os>` on optsize IR), then `llc -O2 -align-loops=1`. Tool: tools/pairs/pin_cg_ablate.py (explicit targets from pin.py). Pin d1106deb71cc. Bytes are the sum of the function symbols' sizes (no padding between functions).

Two instrument limits, found by checking the first run: function names are quoted (`@"_bench_begin@3"`) and attribute groups are numbered anew in each dump (both made passes look like they changed code); a loop pass prints no dump, so its change shows on the next pass that prints (`Loop Strength Reduction` on `Lower Garbage Collection Instructions`).

## Which passes change code (`changed`)

# O2: 77 IRs; pin d1106deb71cc
pass | IRs it changed | functions
X86 DAG->DAG Instruction Selection | 77 | 608
Slot index numbering | 77 | 1216
Merge disjoint stack slots | 77 | 608
Live Variable Analysis | 77 | 600
Eliminate PHI nodes for register allocation | 77 | 608
Two-Address instruction pass | 77 | 608
Register Coalescer | 77 | 601
Greedy Register Allocator | 77 | 1120
Virtual Register Rewriter | 77 | 608
Machine Copy Propagation Pass | 77 | 628
Prologue/Epilogue Insertion & Frame Finalization | 77 | 602
X86 pseudo instruction expansion pass | 77 | 608
Control Flow Optimizer | 75 | 491
Post-RA pseudo instruction expansion pass | 75 | 443
Branch Probability Basic Block Placement | 74 | 426
CodeGen Prepare | 73 | 380
Peephole Optimizations | 72 | 403
Machine code sinking | 67 | 415
Remove unreachable machine basic blocks | 64 | 331
X86 Byte/Word Instruction Fixup | 63 | 385
Lower Garbage Collection Instructions | 62 | 221
Process Implicit Definitions | 62 | 331
Remove dead machine instructions | 61 | 396
Canonicalize natural loops | 59 | 214
X86 Optimize Call Frame | 59 | 383
Early Machine Loop Invariant Code Motion | 58 | 219
Machine Common Subexpression Elimination | 57 | 223
Machine Instruction Scheduler | 55 | 231
Finalize ISel and expand pseudo-instructions | 55 | 218
X86 FP Stackifier | 53 | 266
Stack Slot Coloring | 48 | 156
X86 Fixup SetCC | 41 | 96
X86 LEA Fixup | 37 | 72
Tail Duplication | 33 | 66
Check CFA info and insert CFI instructions if needed | 33 | 85
PostRA Machine Sink | 28 | 49
Early Tail Duplication | 23 | 35
Machine Late Instructions Cleanup Pass | 13 | 20
Machine InstCombiner | 12 | 12
Live Range Shrink | 12 | 14
Shrink Wrapping analysis | 4 | 4
X86 LEA Optimize | 4 | 4
Machine Loop Invariant Code Motion | 2 | 2
X86 EFLAGS copy lowering | 2 | 3
# Os (optsize IR): 77 IRs; pin d1106deb71cc
pass | IRs it changed | functions
X86 DAG->DAG Instruction Selection | 77 | 663
Slot index numbering | 77 | 1326
Merge disjoint stack slots | 77 | 663
Live Variable Analysis | 77 | 655
Eliminate PHI nodes for register allocation | 77 | 663
Two-Address instruction pass | 77 | 663
Register Coalescer | 77 | 658
Greedy Register Allocator | 77 | 1220
Virtual Register Rewriter | 77 | 663
Machine Copy Propagation Pass | 77 | 684
Prologue/Epilogue Insertion & Frame Finalization | 77 | 657
X86 pseudo instruction expansion pass | 77 | 663
Control Flow Optimizer | 75 | 536
Post-RA pseudo instruction expansion pass | 75 | 512
Peephole Optimizations | 72 | 443
CodeGen Prepare | 72 | 411
Machine code sinking | 65 | 441
Lower Garbage Collection Instructions | 64 | 263
Process Implicit Definitions | 63 | 350
X86 Byte/Word Instruction Fixup | 62 | 406
Remove unreachable machine basic blocks | 62 | 361
Branch Probability Basic Block Placement | 62 | 403
Remove dead machine instructions | 61 | 384
Early Machine Loop Invariant Code Motion | 61 | 243
Machine Common Subexpression Elimination | 61 | 230
Canonicalize natural loops | 60 | 237
X86 Optimize Call Frame | 60 | 429
Machine Instruction Scheduler | 54 | 231
Finalize ISel and expand pseudo-instructions | 54 | 221
X86 FP Stackifier | 53 | 280
Stack Slot Coloring | 47 | 139
X86 LEA Fixup | 41 | 84
X86 Fixup SetCC | 40 | 97
X86 LEA Optimize | 34 | 88
PostRA Machine Sink | 30 | 52
Check CFA info and insert CFI instructions if needed | 28 | 80
Tail Duplication | 20 | 27
Early Tail Duplication | 18 | 25
Live Range Shrink | 12 | 13
Machine Late Instructions Cleanup Pass | 12 | 15
Machine InstCombiner | 6 | 6
Shrink Wrapping analysis | 4 | 4
Machine Loop Invariant Code Motion | 3 | 3
X86 EFLAGS copy lowering | 2 | 3

## What the pin's switches cost (`switches`)

Positive is more code with the pass off.

# O2: 77 IRs; pin d1106deb71cc
pass [switch] | d instructions | d bytes | IRs differing | base instructions | base bytes | llc failed
CodeGen Prepare [-disable-cgp] | +4456 | +21337 | 64 | 90170 | 370340 | 0
Loop Strength Reduction [-disable-lsr] | -37 | +40 | 61 | 90170 | 370340 | 0
Constant Hoisting [-disable-constant-hoisting] | +0 | +0 | 0 | 90170 | 370340 | 0
Partially inline calls to library functions [-disable-partial-libcall-inlining] | +0 | +0 | 0 | 90170 | 370340 | 0
Early Tail Duplication [-disable-early-taildup] | -34 | -131 | 9 | 90170 | 370340 | 0
Remove dead machine instructions [-disable-machine-dce] | +11 | +50 | 14 | 90170 | 370340 | 0
Early If-Conversion [-disable-early-ifcvt] | +0 | +0 | 0 | 90170 | 370340 | 0
Early Machine Loop Invariant Code Motion [-disable-machine-licm] | -242 | -1012 | 38 | 90170 | 370340 | 0
Machine Common Subexpression Elimination [-disable-machine-cse] | +342 | +1082 | 53 | 90170 | 370340 | 0
Machine code sinking [-disable-machine-sink] | +699 | +2474 | 64 | 90170 | 370340 | 0
Peephole Optimizations [-disable-peephole] | +2189 | +7527 | 67 | 90170 | 370340 | 0
X86 LEA Optimize [-disable-x86-lea-opt] | +6 | +32 | 3 | 90170 | 370340 | 0
Machine Instruction Scheduler [-enable-misched=0] | +0 | +154 | 31 | 90170 | 370340 | 0
Stack Slot Coloring [-disable-ssc] | +94 | +2880 | 37 | 90170 | 370340 | 0
Machine Copy Propagation Pass [-disable-copyprop] | +122 | +403 | 38 | 90170 | 370340 | 0
Machine Loop Invariant Code Motion [-disable-postra-machine-licm] | +0 | +0 | 0 | 90170 | 370340 | 0
PostRA Machine Sink [-disable-postra-machine-sink] | -1 | +14 | 4 | 90170 | 370340 | 0
Shrink Wrapping analysis [-enable-shrink-wrap=0] | -1 | -1 | 2 | 90170 | 370340 | 0
Control Flow Optimizer [-disable-branch-fold] | +140 | +394 | 40 | 90170 | 370340 | 0
Tail Duplication [-disable-tail-duplicate] | -42 | -193 | 17 | 90170 | 370340 | 0
Post RA top-down list latency scheduler [-disable-post-ra] | +0 | +0 | 0 | 90170 | 370340 | 0
Branch Probability Basic Block Placement [-disable-block-placement] | +286 | +1995 | 64 | 90170 | 370340 | 0
X86 Domain Reassignment Pass [-disable-x86-domain-reassignment] | +0 | +0 | 0 | 90170 | 370340 | 0
X86 cmov Conversion [-x86-cmov-converter=0] | +0 | +0 | 0 | 90170 | 370340 | 0
# Os (optsize IR): 77 IRs; pin d1106deb71cc
pass [switch] | d instructions | d bytes | IRs differing | base instructions | base bytes | llc failed
CodeGen Prepare [-disable-cgp] | +4061 | +19398 | 63 | 77230 | 312407 | 0
Loop Strength Reduction [-disable-lsr] | +283 | +876 | 63 | 77230 | 312407 | 0
Constant Hoisting [-disable-constant-hoisting] | +0 | +0 | 0 | 77230 | 312407 | 0
Partially inline calls to library functions [-disable-partial-libcall-inlining] | +0 | +0 | 0 | 77230 | 312407 | 0
Early Tail Duplication [-disable-early-taildup] | +14 | -4 | 8 | 77230 | 312407 | 0
Remove dead machine instructions [-disable-machine-dce] | +19 | +131 | 14 | 77230 | 312407 | 0
Early If-Conversion [-disable-early-ifcvt] | +0 | +0 | 0 | 77230 | 312407 | 0
Early Machine Loop Invariant Code Motion [-disable-machine-licm] | -304 | -1399 | 42 | 77230 | 312407 | 0
Machine Common Subexpression Elimination [-disable-machine-cse] | +377 | +1151 | 55 | 77230 | 312407 | 0
Machine code sinking [-disable-machine-sink] | +1107 | +3707 | 60 | 77230 | 312407 | 0
Peephole Optimizations [-disable-peephole] | +1763 | +6048 | 69 | 77230 | 312407 | 0
X86 LEA Optimize [-disable-x86-lea-opt] | -60 | +707 | 34 | 77230 | 312407 | 0
Machine Instruction Scheduler [-enable-misched=0] | +93 | +557 | 30 | 77230 | 312407 | 0
Stack Slot Coloring [-disable-ssc] | +6 | +1765 | 33 | 77230 | 312407 | 0
Machine Copy Propagation Pass [-disable-copyprop] | +115 | +353 | 34 | 77230 | 312407 | 0
Machine Loop Invariant Code Motion [-disable-postra-machine-licm] | +0 | +0 | 0 | 77230 | 312407 | 0
PostRA Machine Sink [-disable-postra-machine-sink] | -9 | -28 | 8 | 77230 | 312407 | 0
Shrink Wrapping analysis [-enable-shrink-wrap=0] | +0 | +0 | 0 | 77230 | 312407 | 0
Control Flow Optimizer [-disable-branch-fold] | +134 | +323 | 32 | 77230 | 312407 | 0
Tail Duplication [-disable-tail-duplicate] | +0 | +0 | 0 | 77230 | 312407 | 0
Post RA top-down list latency scheduler [-disable-post-ra] | +0 | +0 | 0 | 77230 | 312407 | 0
Branch Probability Basic Block Placement [-disable-block-placement] | +729 | +2802 | 57 | 77230 | 312407 | 0
X86 Domain Reassignment Pass [-disable-x86-domain-reassignment] | +0 | +0 | 0 | 77230 | 312407 | 0
X86 cmov Conversion [-x86-cmov-converter=0] | +0 | +0 | 0 | 77230 | 312407 | 0

CodeGen Prepare inside, the same inputs:

# O2: 77 IRs; pin d1106deb71cc
pass [switch] | d instructions | d bytes | IRs differing | base instructions | base bytes | llc failed
CodeGen Prepare: branch opts [-disable-cgp-branch-opts] | +0 | +0 | 0 | 90170 | 370340 | 0
CodeGen Prepare: select to branch [-disable-cgp-select2branch] | +2 | +4 | 2 | 90170 | 370340 | 0
CodeGen Prepare: extending-load promotion [-disable-cgp-ext-ld-promotion] | -15 | -180 | 5 | 90170 | 370340 | 0
CodeGen Prepare: store-extract [-disable-cgp-store-extract] | +0 | +0 | 0 | 90170 | 370340 | 0
CodeGen Prepare: delete phis [-disable-cgp-delete-phis] | +0 | +0 | 0 | 90170 | 370340 | 0
CodeGen Prepare: complex address modes [-disable-complex-addr-modes] | +0 | +0 | 0 | 90170 | 370340 | 0
# Os (optsize IR): 77 IRs; pin d1106deb71cc
pass [switch] | d instructions | d bytes | IRs differing | base instructions | base bytes | llc failed
CodeGen Prepare: branch opts [-disable-cgp-branch-opts] | +0 | +0 | 0 | 77230 | 312407 | 0
CodeGen Prepare: select to branch [-disable-cgp-select2branch] | +0 | +0 | 0 | 77230 | 312407 | 0
CodeGen Prepare: extending-load promotion [-disable-cgp-ext-ld-promotion] | -8 | -118 | 6 | 77230 | 312407 | 0
CodeGen Prepare: store-extract [-disable-cgp-store-extract] | +0 | +0 | 0 | 77230 | 312407 | 0
CodeGen Prepare: delete phis [-disable-cgp-delete-phis] | +0 | +0 | 0 | 77230 | 312407 | 0
CodeGen Prepare: complex address modes [-disable-complex-addr-modes] | +0 | +0 | 0 | 77230 | 312407 | 0
