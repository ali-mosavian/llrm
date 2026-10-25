# First divergence from the pin, 71 programs, m32 -O2 (2026-10-25, w13)

Method: our final IR (`--dump`, last stage) through the pin's `llc -O2 -march=i486` (`difftest.normalise`), both asm streams cut to mnemonics (frame push/pop/esp adjusts dropped), first differing window read by hand. `first2.py`, raw windows `out32.txt`. Tree: feat/parity 3c36dac99.

Read this first: the oracle re-runs LSR on our final IR (rows E and F are that instrument, not a defect), and a window of at most seven instructions names a *first* cause only. Rows B and I are the least certain (read, not traced).

| cause | n | programs |
|---|---|---|
| A constant shared by the pin: `xor r,r` / `mov r,imm` before the first compare (CSE2 + the phi copy; PR #4699 plus w10's DAG-isel change) | 15 | fib frames x_fib histo sieve rectwo x_bubble x_insertion x_list x_prefix x_primes x_switch_sparse x_life x_bigadd scroll |
| E FP control-word argument (instrument) | 8 | floats fpbench nbody nbody_single recfloat x_horner x_matmul_f x_nbody_f |
| B prologue / saved-register set / shrink-wrap | 7 | x_hanoi2 x_iir switch x_switch x_switch_dense x_switch_ranges recsum |
| H DAG combine / isel pattern (`sar;shr` for `/2`, 64-bit add carry `setb;add;adc`, magic divide, `shrd`/`shld`, `shl;lea` for `lea [8*r]`) | 7 | x_heapsort x_ll_arith x_rle x_fixmul mandel recgcd particle |
| C entry argument: the pin compares the incoming stack slot (`cmp [esp+N],0`) and reloads; ours keeps it in a register | 6 | x_cordic x_dct x_isqrt x_binsearch bintree x_crc32 |
| F LSR / induction variable (instrument) | 6 | tile x_transpose x_strlen x_reverse x_radix x_base64 |
| I copy coalescing | 4 | recchop recpow x_mergesort x_popcount |
| D scheduler / store order (w12's source-order scheduler) | 3 | lru matmul nbody_fixed |
| G 486 zext row, `xor;mov r8` for `movzx` (recorded, ours faster) | 3 | crc x_histo2 shellsort |
| J spill / allocation decision | 3 | queens quicksort recmany |
| O block placement / loop structure | 3 | x_adler x_collatz x_fir |
| K ebp frame where the pin has none | 2 | x_funcptr x_ll_div |
| N call-argument lowering (`push` against `mov [esp]`, order) | 2 | ring x_hash |
| L zero as `mov r,0` where the pin has `xor` | 1 | hanoi |
| M flags of a shift not reused (`test` after `shr`) | 1 | x_gcd |

Ranked, without the instrument rows: A 15, B 7, H 7, C 6, I 4, then 3s. Biggest non-scheduler cause: A, and half of it waits on the DAG-isel phi copy (w10); the biggest one the allocator owns is C, then B.

m16 (pin `llc` code16 on the same IR) is not done: the pin gets 32-bit address registers there, so its first-divergence needs a per-program IR repair (PLASMA took one by hand).
