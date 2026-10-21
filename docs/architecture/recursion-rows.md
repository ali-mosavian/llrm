# Recursion rows

D10 of [inline cost](../../crates/opt/llrm-transforms/src/inlinecost.rs): a tree-recursive function is copied into itself as gcc's [`recursive_inlining`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.3.0/gcc/ipa-inline.cc#L1645) does, to depth 3 (gcc: 8), 150 `gcc_insns` (gcc: 450, [params.opt](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.3.0/gcc/params.opt#L549)) and the call's frequency over the entry's above 10% ([`want_inline_self_recursive_call_p`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.3.0/gcc/ipa-inline.cc#L991), [L1067](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.3.0/gcc/ipa-inline.cc#L1067)), only where its distinct self-calls average one or more per call (a tree), not at -Os. Measured with the installed gcc 13.4.0 binary's `-Q --help=params`; the cited source is 13.3.0 (same defaults).

Where it runs, and why there: the pin runs the function specializer inside IPSCCP, before its inliner ([PassBuilderPipelines.cpp:1228](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1228), the inliner pipeline at [L1331](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1331)), and tail-call elimination in the function simplification pipeline inside it ([L688](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L688)). Here ipa-cp's second round is the interprocedural step after the walk, so the copies made in the walk enlarged `chop` before ipa-cp priced its clones (recchop -O3: 4 defines against 12, 6368 clocks against 6209). DEVIATE from the pin's TRE slot: a tree-recursive function is marked in the walk (`tre-deferred`), skips tail-call elimination there, and gets its copies and then its elimination at the end of the interprocedural step (`walk::SelfInline`). Linear recursions keep the pin's order (a tail-recursive callee is a loop before its caller inlines it, #3256).

Cleanup: a body that got copies runs the Simplify list twice (`walk::SelfInline`). The list runs GVN before jump threading, and the copies' paths are joined by the threading, so run once each copy repeated the calls of the copies that dominate it (fib -O2, depth 3: 154398 clocks, 1426 spills, 12 calls; run twice 14802 clocks, 0 spills). Rows pair at -O1/-O2/-O3/-Os: fib and x_fib -O2/-O3 154398 to 14802, frames -15.6%, rectwo -0.3%, nothing else changes, QCport identical.

Clocks / bytes of the kernel, i486 model, `tools/rows/row.sh`; main is `854bb7efc`, base is `feat/parity` `3d08fbe4d`. `>` marks a row above main (clocks).

| row | level | main | base | this |
|---|---|---|---|---|
| fib | -O2 | 227171 / 537 | 334157 / 88 > | 154398 / 392 |
| rectwo | -O2 | 612458 / 538 | 1798940 / 104 > | 597725 / 552 |
| x_hanoi2 | -O2 | 206977 / 226 | 251983 / 280 > | 164043 / 1048 |
| queens | -O2 | 175007 / 193 | 161556 / 216 | 161489 / 280 |
| recchop | -O2 | 6955 / 181 | 7729 / 184 > | 6530 / 1112 |
| hanoi | -O2 | 96 / 125 | 188 / 72 > | 180 / 88 > |
| bintree | -O2 | 63391 / 291 | 78382 / 316 > | 77564 / 988 > |
| quicksort | -O2 | 190328 / 343 | 208032 / 348 > | 192307 / 1228 > |
| x_mergesort | -O2 | 65146 / 495 | 91979 / 539 > | 84786 / 907 > |
| recpow | -O2 | 430457 / 888 | 443527 / 299 > | 432936 / 411 > |
| recsum | -O2 | 77291 / 83 | 79296 / 91 > | 79296 / 91 > |
| fib | -O3 | 227171 / 537 | 334157 / 88 > | 154398 / 392 |
| rectwo | -O3 | 612458 / 538 | 1798940 / 104 > | 604421 / 552 |
| x_hanoi2 | -O3 | 134926 / 1475 | 251983 / 280 > | 164043 / 1048 > |
| queens | -O3 | 94766 / 2670 | 152238 / 789 > | 151838 / 757 > |
| recchop | -O3 | 5946 / 1450 | 6209 / 5402 > | 5932 / 5113 |
| hanoi | -O3 | 96 / 125 | 188 / 72 > | 180 / 88 > |
| bintree | -O3 | 58251 / 1086 | 75992 / 332 > | 75189 / 1004 > |
| quicksort | -O3 | 178429 / 1697 | 213378 / 564 > | 200374 / 1588 > |
| x_mergesort | -O3 | 65146 / 495 | 91979 / 539 > | 84910 / 955 > |
| recpow | -O3 | 430457 / 888 | 434368 / 315 > | 424974 / 427 |
| recsum | -O3 | 77291 / 83 | 79296 / 91 > | 79296 / 91 > |

No row is above base in clocks at any level (34 below, 10 equal of the 44 rows of the recursion programs at -O1/-O2/-O3/-Os); recchop -O3 is under main (5932 against 5946). Clocks go down and bytes go up on the rows that gain: fib 537 to 392 bytes is both, x_hanoi2 and recchop trade 4-7x bytes for clocks, and the three loop-bearing rows (bintree, quicksort, x_mergesort) pay 3-5x main's bytes for 0-8% clocks against base.

Limits, depth 3 (before the move above): budget 60 / 100 / 150 / 450 `gcc_insns` give fib 207179 / 132006 / 132006 / 132006 and rectwo 1489763 / 876030 / 530081 / 530081, so 150 is the smallest that keeps both under main. gcc's depth 8 and 450 give fib 227940 / 1736 B and bintree 2876 B. Differences from gcc: the copies are made depth first, gcc's by a heap of edges ordered by badness; gcc's `peeling` and cold-edge checks are not translated.

Peeling (D10, second half): a recursive callee inlined into another function has its own call inlined in the copy, as gcc's [`inline_small_functions`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.3.0/gcc/ipa-inline.cc#L2237) does for an edge whose caller is a copy of the callee. [`want_inline_self_recursive_call_p`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.3.0/gcc/ipa-inline.cc#L1033) compares the edge's frequency with that of the call that made the outermost copy, which is the calls to itself per call to the power of the copies deep, with 1 - 1/8 squared once for each copy before it where the body has more than one call to itself (DEVIATE: their copies grow as the calls to the power of the depth, which the squaring bounds; with one call they grow by one a level, and without the squaring hanoi goes from 133 to 103 clocks, 152 to 184 bytes, main 96 / 125, and no row is worse than with it); here the copy is peeled while that is below the limit, a copy adds at most `max-inline-insns-auto` (15) `gcc_insns`, and the copies of one call add at most 450, gcc's recursive growth limit (DEVIATE: gcc has no such cap on peeling; two calls in exclusive branches double the copies at each level, recpow -O1 32 copies and 8203 bytes); not at -Os. DEVIATE: gcc reads the one edge, here the function's calls to itself per call, so a tree (fib 1, bintree 1.25) is never peeled (peeling every edge doubled bintree's copy at each level: -O2 77666 against 77564 clocks, 988 to 1276 bytes).

OPEN, above main and not made worse than base by this change, with what the assembly of main and of this shows (`-S`, -O2, i486):

- **hanoi** (96 against 103; owner w-6, 2026-10-30): main peels the linear recursion `2 * hanoi(n-1) + 1` five levels into compare-and-add chains (+53 bytes). Peeling is in (above), eight levels here (180 to 163 to 133 to 103 clocks, 88 to 184 bytes); what is left is the shape of the copies, not their depth.
- **recsum** (79296 against 77291; owner w-5, 2026-10-30): no recursion left. Main's loop exits on the scaled counter (`add ebx, -3; jne`); this keeps `n` as a second counter (`dec ebx`) beside `3n`, one more register and instruction a trip. Linear function test replacement of an induction variable used only by the exit test: LSR row #3075.
- **quicksort** and **x_mergesort** (owner splitkit-split-cost-row, 2026-11-06): at base and in main the calls and blocks are the same shape (4 calls and 142 / 155 lines for quicksort); main's gap is the partition and merge loop bodies, not the recursion. Backend loop code (spills in the nest): the regalloc and splitkit rows.
- **bintree** (owner splitkit-split-cost-row, 2026-11-06): main turns the last call into a loop (`jne` back to the entry, accumulator in memory) and keeps one call, 291 bytes; base does the same at 316 bytes and 78382 clocks, so the gap is the body's frame (main 8 bytes and 4 pushes, this 32 bytes and 4 pushes), again register allocation.

recpow was +3.7% at -O2 until the tree term (0.625 calls per call) left it alone.
