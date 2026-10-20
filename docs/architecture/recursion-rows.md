# Recursion rows

D10 of [inline cost](../../crates/opt/llrm-transforms/src/inlinecost.rs): a recursive function is copied into itself as gcc's `recursive_inlining` does. Clocks / bytes of the kernel, i486 model, `tools/rows/row.sh`; main is `854bb7efc`, base is `feat/parity` before this change.

| row -O2 | main | base | this |
|---|---|---|---|
| fib | 227171 / 537 | 334157 / 88 | 132006 / 360 |
| rectwo | 612458 / 538 | 1798940 / 104 | 530081 / 520 |
| x_hanoi2 | 206977 / 226 | 251983 / 280 | 165385 / 968 |
| queens | 175007 / 193 | 161556 / 216 | 161489 / 280 |
| recchop | 6955 / 181 | 7729 / 184 | 6942 / 1272 |
| hanoi | 96 / 125 | 188 / 72 | 180 / 88 |
| bintree | 63391 / 291 | 78382 / 316 | 77655 / 908 |
| quicksort | 190328 / 343 | 208032 / 348 | 193476 / 1148 |
| x_mergesort | 65146 / 495 | 91979 / 539 | 82849 / 1579 |
| recpow | 430457 / 888 | 443527 / 299 | 460022 / 555 |
| recsum | 77291 / 83 | 79296 / 91 | 79296 / 91 |

Limits, depth 3: budget 60 / 100 / 150 / 450 `gcc_insns` give fib 207179 / 132006 / 132006 / 132006 and rectwo 1489763 / 876030 / 530081 / 530081, so 150 is the smallest that keeps both under main. gcc's depth 8 and 450 give fib 227940 / 1736 B and bintree 2876 B.

Open: hanoi (main folds it further), bintree, quicksort, x_mergesort (loop-bearing recursion: main's old passes win), recpow (two calls in exclusive branches count as two, so a linear recursion is peeled: +3.7% against base), recsum.
