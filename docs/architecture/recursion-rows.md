# Recursion rows

D10 of [inline cost](../../crates/opt/llrm-transforms/src/inlinecost.rs): a recursive function is copied into itself as gcc's `recursive_inlining` does, to depth 3, 150 `gcc_insns` and 10% probability, only where its distinct self-calls average one or more per call (a tree), not at -Os. Clocks / bytes of the kernel, i486 model, `tools/rows/row.sh`; main is `854bb7efc`, base is `feat/parity` `2db003b9b`. `>` marks a row above main (clocks).

| row | level | main | base | this |
|---|---|---|---|---|
| fib | -O2 | 227171 / 537 | 334157 / 88 > | 132006 / 360 |
| rectwo | -O2 | 612458 / 538 | 1798940 / 104 > | 530081 / 520 |
| x_hanoi2 | -O2 | 206977 / 226 | 251983 / 280 > | 165385 / 968 |
| queens | -O2 | 175007 / 193 | 161556 / 216 | 161489 / 280 |
| recchop | -O2 | 6955 / 181 | 7729 / 184 > | 6942 / 1272 |
| hanoi | -O2 | 96 / 125 | 188 / 72 > | 180 / 88 > |
| bintree | -O2 | 63391 / 291 | 78382 / 316 > | 77655 / 908 > |
| quicksort | -O2 | 190328 / 343 | 208032 / 348 > | 193476 / 1148 > |
| x_mergesort | -O2 | 65146 / 495 | 91979 / 539 > | 82849 / 1579 > |
| recpow | -O2 | 430457 / 888 | 443527 / 299 > | 432936 / 411 > |
| recsum | -O2 | 77291 / 83 | 79296 / 91 > | 79296 / 91 > |
| fib | -O3 | 227171 / 537 | 334157 / 88 > | 132006 / 360 |
| rectwo | -O3 | 612458 / 538 | 1798940 / 104 > | 537017 / 520 |
| x_hanoi2 | -O3 | 134926 / 1475 | 251983 / 280 > | 165385 / 968 > |
| queens | -O3 | 94766 / 2670 | 152238 / 789 > | 151838 / 757 > |
| recchop | -O3 | 5946 / 1450 | 6209 / 5402 > | 6368 / 1944 > |
| hanoi | -O3 | 96 / 125 | 188 / 72 > | 180 / 88 > |
| bintree | -O3 | 58251 / 1086 | 75992 / 332 > | 75262 / 924 > |
| quicksort | -O3 | 178429 / 1697 | 213378 / 564 > | 192307 / 1228 > |
| x_mergesort | -O3 | 65146 / 495 | 91979 / 539 > | 83022 / 1659 > |
| recpow | -O3 | 430457 / 888 | 434368 / 315 > | 424974 / 427 |
| recsum | -O3 | 77291 / 83 | 79296 / 91 > | 79296 / 91 > |

Clocks go down and bytes go up on the rows that gain: fib 227171 / 537 to 132006 / 360 is both, x_hanoi2 and recchop trade 4-7x bytes for clocks, and the three loop-bearing rows (bintree, quicksort, x_mergesort) pay 3-5x main's bytes for 0-8% clocks against base.

Limits, depth 3: budget 60 / 100 / 150 / 450 `gcc_insns` give fib 207179 / 132006 / 132006 / 132006 and rectwo 1489763 / 876030 / 530081 / 530081, so 150 is the smallest that keeps both under main. gcc's depth 8 and 450 give fib 227940 / 1736 B and bintree 2876 B.

OPEN, above main and not made worse than base by this change, with what the assembly of main and of this shows (`-S`, -O2, i486):

- **hanoi** (96 against 180): main peels the linear recursion `2 * hanoi(n-1) + 1` five levels into compare-and-add chains (+53 bytes); this refuses it (one distinct self-call, and the recpow rule). A cost rule, not a shape: peel a linear recursion when the copied instructions are fewer than the call overhead they save (call, return, argument and stack traffic, from the i486 table). Needs the probability product (gcc: `min-inline-recursive-probability`) to say how deep.
- **recsum** (79296 against 77291): no recursion left. Main's loop exits on the scaled counter (`add ebx, -3; jne`); this keeps `n` as a second counter (`dec ebx`) beside `3n`, one more register and instruction a trip. Linear function test replacement of an induction variable used only by the exit test: LSR row #3075 (hir-branch-icmp-2211).
- **quicksort** (193476 against 190328) and **x_mergesort** (82849 against 65146): at base and in main the calls and blocks are the same shape (4 calls and 142 / 155 lines for quicksort); main's gap is the partition and merge loop bodies, not the recursion. Backend loop code (spills in the nest): the regalloc and splitkit rows.
- **bintree** (77655 against 63391): main turns the last call into a loop (`jne` back to the entry, accumulator in memory) and keeps one call, 291 bytes; base does the same at 316 bytes and 78382 clocks, so the gap is the body's frame (main 8 bytes and 4 pushes, this 32 bytes and 4 pushes), again register allocation. This change copies the tree three levels before that (908 bytes) and gains 1%.

R1, row worse than base: recchop -O3, 6368 against 6209 clocks (+2.6%), bytes 5402 to 1944. The base reaches it with `ipa-cp-clone` of the recursive calls; here the copies come first, and gcc runs ipa-cp before its inliner. Deferring the copies of functions another passes a constant to gave recchop 6273 (+1.0%) but lost x_hanoi2 -O3 (165385 to 251939), bintree and quicksort; copies after the ipa-cp stage see a loop with one self-call (tail-call elimination ran in the walk) and fib stays at 294986.

F1, follow-up (owner w-6, 2026-10-30): move tail-call elimination of a recursive function out of the walk's Simplify list to after the interprocedural step, as gcc's `pass_tail_recursion` runs after `pass_ipa_inline` and `pass_ipa_cp`; then the self-copies can follow ipa-cp and see the tree. Price it as an isolated POC on fib, recchop and queens -O3 first.

recpow was +3.7% at -O2 until the tree term (0.625 calls per call) left it alone: 432936 against 443527.
