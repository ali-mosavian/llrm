# LSR address-register term (LU2): tried, not landed

Owner: cycles-queue-quiet-host. 2026-10-22. Branch `perf/lsr-address-registers`, draft #3585.

Why: 7 functions of 9385 (QCport m16: `ls.c` `_ls_animate@3` at five levels, `sc.c` `_sc_bsplit@3` at -O2/-O3) fail the greedy allocator at stage Done and are placed by the rounds loop. The 8086 addresses through BX, SI and DI; at the `pattern[nf]` load of `_ls_animate` four address values are live (the `ls` pointer, the strength-reduced index, a loaded pointer, an `idiv` remainder). The pin has no 8086 model (its code16 uses 32-bit registers and scaled addressing), so there is no oracle.

The term (DEVIATE LU2, `lsr_use.rs`): `Tti::address_registers` (m16: bases + indices of the native address form = 3); an address use naming a register past `3 - (loop-variant leaves of the loop's address chains)` pays 2 instructions (a reload). Narrowed: only where a two-register formula certainly overflows.

Result (dist pairs, parity 3ce2ce70ab..): `ls.c` 0 failures at all levels (5 of 7 fixed), `sc.c` unchanged. m32 identical. m16 bytes: quicksort +27..+716 (-O2 +7.7%), matmul +44..+141, shellsort -O3 +127; first form also fpbench +9%. QCport -O1 +0.077%, -O2 +0.025%, -O3 +0.021%, -Os -0.048%; modules over the bar: sc -O1 +1.6%, part, bench.

Not landing: the term picks a pointer induction plus counter in loops the greedy places fine, so it trades 7 functions for hundreds of bytes. The pin's answer is region splitting in the greedy (SplitKit); splitkit's ladder check 10-24 decides. Do not rediscover: a failure predictor would have to ask the allocator, not estimate pressure in LSR.
