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

OPEN, above main and not made worse than base by this change (mechanism still to find): hanoi (main folds it further), bintree, quicksort, x_mergesort (loop-bearing recursion, main's old passes win), recsum.

Worse than base: recchop -O3, 6368 against 6209 clocks (+2.6%) at 1944 bytes against 5402. The base's -O3 reaches it by `ipa-cp-clone` of the recursive calls (gcc runs ipa-cp before its inliner, here the copies come first). recpow was +3.7% at -O2 until the tree term (0.625 calls per call) left it alone: 432936 against 443527.
