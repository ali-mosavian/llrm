# llrm m32 against gcc and clang

**TL;DR:** on the 22 C benchmarks llrm executes 1.38x the instructions and 1.37x the 486 clocks of the better of gcc 13.4 / clang 20.1 at `-O2` (geomean; medians 1.27x and 1.18x); at `-Os` 1.29x and 1.36x. It beats both on 3 programs by 10% or more (matmul, sieve, particle) and loses 2x or more on instructions or clocks on 6 (bintree, hanoi, queens, scroll, fib, frames), for eight general reasons below.

## Method

- llrm: `llrm-c --target x86-m32 -O2 -march=i486 -fno-inline-functions`. gcc/clang: `-m32 -march=i486 -fno-pic -fno-inline-functions -fno-stack-protector -fcf-protection=none`, `-O2` and `-Os`. Same `-fno-inline-functions` as tools/bench; the kernel carries `noinline` for gcc/clang, which otherwise inline it into `main` (llrm does not).
- One emulator (unicorn) for all: llrm's OMF is linked in Python, gcc/clang's ELF by `ld`; both resolve `report`/`memset`/`memcpy`/`memmove` to the same stub. The whole program runs from `main`; the kernel `bench_X` is counted from entry to its own return, callees included. Counting follows tools/bench/icount.py (a `rep` instruction counts once per iteration; memory operands exclude `lea`). Clock estimates use a 486 table (`cost()` in harness.py), without pipeline effects.
- Not counted: alignment `nop`s (clang pads loops with them, e.g. 94710 in fib), reported in the `nops` column of table.md.
- Skipped: `grep` (10 MB input, timed only), `huge`, `textfill` (16-bit only). `lru` is built with `-Dfar=` for gcc/clang. `parity` is not a program.
- Rerun: `crates/target/llrm-x86-m32/vsgcc/run.sh` (see `crates/target/llrm-x86-m32/vsgcc/readme.md`); it builds llrm-c and reproduces every table here. The listings that back each finding are in the appendix.

## Instrument check

- llrm sieve/crc/matmul = 12112/478/2447, identical to the old loops32 `count.py`.
- Native i386 under gdb `stepi` (real silicon): sieve gcc -O2 14891 = harness; clang 14669 vs 14668 and llrm 12113 vs 12112 (the extra step is a `rep` with `ecx=0`, which icount.py skips).
- Every run's `report()` values equal `bench/NAME.out` (test_harness.py).
- Two instrument faults found and fixed on the way: gcc/clang scroll gave 32636400, not 32634864, because my `memmove` stand-in copied forwards over an overlap; gcc inlined three kernels into `main`, reading 0 instructions. Both have tests in `test_harness.py`.

- The clock column is estimated, and the multiply is its one data-dependent term: the 486's `MUL`/`IMUL` early-out
  reads the multiplier's most significant bit, `10 + max(bits of |m|, n)` clocks, n = 3 for a positive and 5 for a
  negative multiplier (Intel 240440-002, Nov 1989 i486 data sheet, Table 10.1, PDF p.135 and printed p.143 note 3;
  https://bitsavers.trailing-edge.com/components/intel/80486/240440-002_i486_Microprocessor_Nov89.pdf). The data
  sheet names the multiplier beside the register or memory operand, not the accumulator, so `harness.multiplier_of`
  reads the r/m source of `imul r/m32` and the immediate of the three-operand form. recmany shows what rests on it:
  gcc's reciprocal multiply holds the magic number in EAX and the numerator in the r/m operand (about 14 clocks),
  clang's the reverse (10 + 32 = 42, the same as `idiv`). The rounding of the logarithm is not stated, see
  docs/measurement/timing-audit.md; a program whose ratio rests on an `imul` is not read as a compiler finding
  until its operand order is.

## Table (-O2; -Os and memory operands in table.md)

| program | llrm ins | gcc | clang | llrm/best | llrm clk | gcc | clang | llrm/best | llrm B | gcc B | clang B |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| bintree | 102097 | 44110 | 37533 | 2.72 | 147928 | 63767 | 61715 | 2.40 | 363 | 406 | 349 |
| crc | 478 | 656 | 436 | 1.10 | 588 | 874 | 627 | 0.94 | 194 | 109 | 205 |
| fib | 317419 | 206373 | 234042 | 1.54 | 558228 | 288771 | 329970 | 1.93 | 87 | 115 | 109 |
| floats | 7022 | 8021 | 7018 | 1.00 | 113082 | 114086 | 113087 | 1.00 | 129 | 156 | 141 |
| fpbench | 21268 | 19838 | 15132 | 1.41 | 193874 | 164977 | 155456 | 1.25 | 1650 | 511 | 1725 |
| frames | 3807974 | 2842883 | 3702021 | 1.34 | 12147645 | 5491071 | 6622945 | 2.21 | 164 | 207 | 214 |
| hanoi | 286704 | 143356 | 249852 | 2.00 | 696273 | 208890 | 348155 | 3.33 | 114 | 115 | 157 |
| histo | 76054 | 87702 | 71326 | 1.07 | 170780 | 177821 | 177856 | 0.96 | 175 | 199 | 221 |
| lru | 119 | 89 | 85 | 1.40 | 170 | 127 | 118 | 1.44 | 526 | 515 | 509 |
| mandel | 203578 | 166784 | 167607 | 1.22 | 624373 | 529091 | 558041 | 1.18 | 163 | 193 | 214 |
| matmul | 2447 | 6448 | 2903 | 0.84 | 9962 | 17570 | 11689 | 0.85 | 2489 | 402 | 1789 |
| nbody | 60677 | 47851 | 52070 | 1.27 | 433232 | 378453 | 408048 | 1.14 | 733 | 422 | 733 |
| nbody_fixed | 1341164 | 1129310 | 1522118 | 1.19 | 4499186 | 4315106 | 4610126 | 1.04 | 1688 | 573 | 1190 |
| nbody_single | 1115127 | 997142 | 813153 | 1.37 | 10085884 | 8483770 | 8175991 | 1.23 | 1439 | 567 | 1430 |
| particle | 84644 | 94743 | 94241 | 0.90 | 167153 | 176551 | 237249 | 0.95 | 241 | 272 | 278 |
| queens | 245602 | 117173 | 178963 | 2.10 | 358122 | 154367 | 231701 | 2.32 | 225 | 257 | 260 |
| quicksort | 155863 | 152574 | 144575 | 1.08 | 240874 | 212749 | 206721 | 1.17 | 337 | 397 | 349 |
| ring | 40105 | 41132 | 46103 | 0.98 | 60157 | 61182 | 78151 | 0.98 | 83 | 136 | 102 |
| scroll | 1196306 | 176970 | 179958 | 6.76 | 1820006 | 507670 | 515108 | 3.59 | 184 | 266 | 238 |
| shellsort | 8513 | 9256 | 8002 | 1.06 | 13963 | 12236 | 13308 | 1.14 | 227 | 275 | 621 |
| sieve | 12112 | 14891 | 14558 | 0.83 | 19262 | 20986 | 26191 | 0.92 | 186 | 296 | 189 |
| tile | 7157 | 5259 | 4210 | 1.70 | 11159 | 9286 | 8287 | 1.35 | 137 | 184 | 147 |

Geomean llrm/best: -O2 1.38 ins, 1.37 clk, 1.22 code; -Os 1.29 ins, 1.36 clk, 1.02 code.

## Findings

Each is a general gap; the instruction deltas are from the hottest loops (`loops.PROG.txt`).

1. **No calling convention for private functions.** A `static` function takes its arguments on the stack, builds `push ebp; mov ebp,esp … leave; ret 4`, and spills across the recursive call. gcc/clang pass the argument in a register and drop the frame. fib: 65672 memory operands against 1; 317k instructions against 206k/234k. Also queens (the `col` argument is re-read from `[ebp+10h]` inside `safe`'s loop), bintree, hanoi. Owner: backend selection (m32-prep).
2. **Tail recursion is not turned into a loop.** bintree's `insert` and hanoi's second call stay calls: 102097 instructions against 37–44k, hanoi 286704 against gcc 143356. gcc also folds fib's second call into a loop with an accumulator. MIR transform.
3. **Division selection ignores sign and range facts.** `rest > 0` makes `rest / 10` and `rest % 10` unsigned: gcc/clang use `mul; shr` (frames: 10 instructions, 5.5M clk); llrm uses `cdq; idiv` (7 instructions, 12.1M clk, 2.2x). The 486 table rejects the signed reciprocal (`timing.rs` test), but an unsigned one wins. nbody_fixed: gcc proves `dist2 >= 0` (sum of squares, `nsw`) and divides by a power of two with `sar`; llrm adds the 5-instruction bias (`mov; sar 31; and; add; sar`) at every division. Analysis (known-bits / non-negative facts → selection): compile-time for the analysis, m32-prep for the selection.
4. **No copy-loop idiom.** `for (i…) a[i] = a[i+80]` and its backward twin become `rep movsd` in gcc/clang (1 instruction per 2 words); llrm copies a word per 4-instruction iteration. scroll: 1196306 against 177k–180k instructions, 6.8x.
5. **Induction variables are not simplified.**
   - Derived values are recomputed: queens' `row - r` and `r - row` cost `mov; sub; cmp` (3) each iteration; gcc/clang step a register (1 + `cmp`).
   - Masks the range makes redundant stay: tile's `(x+7)&63` with `x < 40` is `mov; and` per element, where gcc/clang use a plain pointer or a count-to-zero scaled index (7 against 5 and 4 instructions).
   - Narrow counters are re-extended every iteration: mandel `inc cx; movsx eax,cx; cmp cx,20h`, nbody `inc bx; movzx esi,bx; cmp bx,4`. A 32-bit counter needs neither.
   Programs: queens, tile, mandel, nbody, fpbench. Analysis (induction/SCEV, range) plus MIR; compile-time for the analysis part.
6. **x87 code: invariants and constants are re-pushed, shuffled with `fxch`.** The `x[i]`/`y[i]` loads stay inside the `j` loop (gcc keeps them on the stack under the loop); `fld1`/`fldz` are re-pushed at each use where gcc reuses one with `fadd st(1),st`; an operand loaded for a later `fsubr mem` is first copied (`fld st,st(0); fsubr m; fxch`, 3 instructions where gcc has `fld m; fsub m`); `fxch; fxch` pairs and `fldz; faddp` remain. nbody 38 instructions per iteration against 27; fpbench and nbody_single 1.25–1.4x. Backend (m32-prep): x87 stack allocation and scheduling.
7. **Constant multiplication is expanded past the cost of `imul`.** bintree's LCG `x * 25173` is a 14-instruction shift/add chain every iteration; gcc/clang emit `imul r,r,0x6255` (13 clk on the 486). Also mandel and nbody_fixed use `mov x,y; add x,z` where gcc/clang use `lea x,[y+z]` (one instruction less, and `lea [a+b+disp]` folds a third add). Instruction choice: m32-prep.
8. **Loop invariants live in memory while the loop stores to a slot.** mandel's loop stores `[ebp-10h]` every iteration and reads `cx`/`cy` from the frame: 2.88 memory operands per iteration against 0.94–1.0. Register allocation under pressure (spill choice); compile-time/regalloc owner.

## Wins

- **Count-to-zero loops with flag reuse**: `add reg,2; jne` or `inc; jne` with no `cmp` (particle 8 instructions per iteration against 9, scroll, quicksort, matmul). gcc keeps `cmp` + `jne` in particle and sieve.
- **Full unrolling of constant-trip inner loops** (matmul 2447 against 6448 for gcc and 2903 for clang; crc against gcc). It costs code: matmul is 2489 bytes against 402.
- **Narrow data without extension** (sieve): the 16-bit index feeds the address through `ebx` without a per-iteration `movzx`/`xor` (4 instructions against 5 and 6).
- llrm's `-Os` code is the same size as the best of gcc/clang (geomean 1.02).

## Compile time

Best of 5 wall-clock runs, source to object, one process per run (ms, geomean of 22): llrm -O2 38.7, -Os 29.6; gcc -O2 10.3, -Os 9.4; clang -O2 16.1, -Os 13.6. Per program in `ctime.json`. Outliers at llrm -O2: matmul 525 ms (80 at -Os), nbody_fixed 437 (77), fpbench 133, nbody_single 124 ; on matmul, `mir gvn` is 442 of 540 ms (11 runs, `LLRM_DEBUG=time`), on the unrolled body. For compile-time.

## Caveats

- Clock estimates are a table, not a cycle-accurate model: no AGI stalls, prefix penalties (llrm and gcc both use 16-bit operations in m32, a +1 clock prefix on the 486) or cache effects.
- `memset`/`memcpy` of gcc/clang runs in a `rep stosd/movsd` stand-in, counted per iteration as in tools/bench; a real libc would differ.
- `lru` is tiny (119 instructions); its ratio is noise.

## Appendix: hottest-loop listings

Columns: executions per iteration of the loop header (`*` fewer, blank equal). Header lines give the loop's share of the kernel's instructions.

### queens
```
## queens llrm: header 0x10018, 9297 iterations, 13.02 instructions and 2.00 memory operands per iteration, 49% of 245602
     1.00  mov esi,[eax]
     1.00  sub esi,[ebp+10h]
     1.00  or esi,esi
     1.00  je short 00010045h
  *  0.83  mov edi,ecx
  *  0.83  sub edi,ebx
  *  0.83  cmp esi,edi
  *  0.83  je short 00010045h
  *  0.75  mov edi,ebx
  *  0.75  add edi,edx
  *  0.75  cmp esi,edi
  *  0.75  je short 00010045h
  *  0.67  inc ebx
  *  0.67  add eax,4
  *  0.67  cmp ebx,ecx
  *  0.67  jl short 00010018h
```
```
## queens gccO2: header 0x1007d, 9297 iterations, 8.34 instructions and 1.00 memory operands per iteration, 66% of 117173
  *  0.83  mov ecx,eax
  *  0.83  add ecx,edx
  *  0.83  je short 00010084h
  *  0.75  cmp eax,edx
  *  0.75  je short 00010084h
  *  0.67  inc edx
  *  0.67  je short 000100A0h
     1.00  mov eax,[esi+edx*4]
     1.00  sub eax,ebx
     1.00  jne short 00010070h
```
```
## queens clangO2: header 0x100e0, 9297 iterations, 8.86 instructions and 1.00 memory operands per iteration, 46% of 178963
     1.00  mov ebx,[ecx]
     1.00  sub ebx,esi
     1.00  je short 000100FAh
  *  0.83  cmp edx,ebx
  *  0.83  je short 000100FAh
  *  0.75  cmp edi,ebx
  *  0.75  je short 000100FAh
  *  0.67  add ecx,4
  *  0.67  inc edi
  *  0.67  dec edx
  *  0.67  jne short 000100E0h
```

### tile
```
## tile llrm: header 0x10016, 1000 iterations, 7.00 instructions and 1.00 memory operands per iteration, 98% of 7157
     1.00  mov esi,ebx
     1.00  and esi,7Eh
     1.00  movsx esi,word ptr [edx+esi+10090h]
     1.00  add eax,esi
     1.00  add ebx,2
     1.00  cmp ebx,5Eh
     1.00  jne short 00010016h
```
```
## tile gccO2: header 0x100a0, 1000 iterations, 5.00 instructions and 1.00 memory operands per iteration, 95% of 5259
     1.00  movsx ecx,word ptr [eax]
     1.00  add edx,ecx
     1.00  add eax,2
     1.00  cmp ebx,eax
     1.00  jne short 000100A0h
```
```
## tile clangO2: header 0x10020, 1000 iterations, 4.00 instructions and 1.00 memory operands per iteration, 95% of 4210
     1.00  movsx ebx,word ptr [esi+edi*2+1105Eh]
     1.00  add eax,ebx
     1.00  inc edi
     1.00  jne short 00010020h
```

### frames
```
## frames llrm: header 0x10049, 138894 iterations, 7.00 instructions and 0.00 memory operands per iteration, 26% of 3807974
     1.00  mov eax,ebx
     1.00  cdq
     1.00  idiv esi
     1.00  mov ebx,eax
     1.00  add ecx,edx
     1.00  or ebx,ebx
     1.00  jg short 00010049h
```
```
## frames gccO2: header 0x100a0, 138894 iterations, 10.00 instructions and 0.00 memory operands per iteration, 49% of 2842883
     1.00  mov eax,ecx
     1.00  mul esi
     1.00  shr edx,3
     1.00  lea eax,[edx+edx*4]
     1.00  add eax,eax
     1.00  sub ecx,eax
     1.00  add ebx,ecx
     1.00  mov ecx,edx
     1.00  test edx,edx
     1.00  jne short 000100A0h
```
```
## frames clangO2: header 0x10050, 138894 iterations, 11.00 instructions and 0.00 memory operands per iteration, 41% of 3702021
     1.00  mov eax,esi
     1.00  mul edi
     1.00  shr edx,3
     1.00  lea eax,[edx+edx]
     1.00  lea eax,[eax+eax*4]
     1.00  mov ebx,esi
     1.00  sub ebx,eax
     1.00  add ecx,ebx
     1.00  cmp esi,9
     1.00  mov esi,edx
     1.00  ja short 00010050h
```

### scroll
```
## scroll llrm: header 0x1004c, 100000 iterations, 4.00 instructions and 2.00 memory operands per iteration, 33% of 1196306
     1.00  mov dx,[ecx+12000h]
     1.00  mov [ecx+11060h],dx
     1.00  add ecx,2
     1.00  jne short 0001004Ch
```
```
## scroll gccO2: header 0x100d6, 50000 iterations, 1.00 instructions and 2.00 memory operands per iteration, 28% of 176970
     1.00  rep movsd [edi],[esi]
```
```
## scroll clangO2: header 0x8067, 98000 iterations, 1.00 instructions and 2.00 memory operands per iteration, 54% of 179958
     1.00  rep movsd [edi],[esi]
```

### bintree
```
## bintree llrm: header 0x100dd, 300 iterations, 32.98 instructions and 5.99 memory operands per iteration, 10% of 102097
     1.00  movzx ecx,bx
     1.00  mov ebx,ecx
     1.00  add ebx,ebx
     1.00  add ebx,ecx
     1.00  shl ebx,4
     1.00  add ebx,ecx
     1.00  shl ebx,3
     1.00  add ebx,ecx
     1.00  shl ebx,2
     1.00  add ebx,ecx
     1.00  shl ebx,2
     1.00  add ebx,ecx
     1.00  shl ebx,2
     1.00  add ebx,ecx
     1.00  add ebx,3619h
     1.00  movzx ecx,bx
     1.00  and ecx,0FFFh
     1.00  mov edx,[ebp-8]
     1.00  mov [ebp+edx-8],ecx
     1.00  cmp eax,1
     1.00  jle short 0001012Fh
  *  1.00  push eax
  *  1.00  push 1
  *  1.00  lea ecx,[ebp-0E24h]
  *  1.00  push ecx
  *  1.00  mov [ebp-4],eax
  *  1.00  call 00010000h
  *  1.00  mov eax,[ebp-4]
     1.00  inc eax
     1.00  mov ecx,[ebp-8]
     1.00  add ecx,0Ch
     1.00  mov [ebp-8],ecx
     1.00  jne short 000100DDh
```
```
## bintree gccO2: header 0x10150, 2492 iterations, 11.00 instructions and 2.00 memory operands per iteration, 62% of 44110
     1.00  lea ebp,[eax+eax]
     1.00  add eax,ebp
     1.00  xor ecx,ecx
     1.00  cmp edx,[esp+eax*4+14h]
     1.00  setge cl
     1.00  mov ebp,ecx
     1.00  lea eax,[ebp+eax+1]
     1.00  lea ebp,[ebx+eax*4]
     1.00  mov eax,[ebp]
     1.00  test eax,eax
     1.00  jne short 00010150h
```
```
## bintree clangO2: header 0x100c0, 2492 iterations, 8.00 instructions and 2.00 memory operands per iteration, 53% of 37533
     1.00  lea edx,[edx+edx*2]
     1.00  lea edi,[ecx+edx*4]
     1.00  xor ebx,ebx
     1.00  cmp esi,[ecx+edx*4]
     1.00  setge bl
     1.00  mov edx,[edi+ebx*4+4]
     1.00  test edx,edx
     1.00  jne short 000100C0h
```

### mandel
```
## mandel llrm: header 0x10030, 9449 iterations, 20.39 instructions and 2.88 memory operands per iteration, 95% of 203578
     1.00  mov esi,edx
     1.00  imul esi,edx
     1.00  sar esi,8
     1.00  mov edi,ebx
     1.00  imul edi,ebx
     1.00  sar edi,8
     1.00  mov [ebp-10h],eax
     1.00  mov eax,esi
     1.00  add eax,edi
     1.00  cmp eax,400h
     1.00  jg short 0001006Bh
  *  0.94  imul ebx,edx
  *  0.94  sar ebx,7
  *  0.94  add ebx,[ebp-4]
  *  0.94  mov edx,esi
  *  0.94  sub edx,edi
  *  0.94  add edx,[ebp-0Ch]
  *  0.94  inc cx
  *  0.94  movsx eax,cx
  *  0.94  cmp cx,20h
  *  0.94  jl short 00010030h
```
```
## mandel gccO2: header 0x10060, 8873 iterations, 17.81 instructions and 1.00 memory operands per iteration, 95% of 166784
     1.00  imul ebx,eax
     1.00  sar ebx,7
     1.00  mov eax,[esp]
     1.00  add ebx,eax
     1.00  sub edx,ecx
     1.00  lea eax,[edx+ebp]
     1.00  inc esi
     1.00  cmp si,20h
     1.00  je short 000100C0h
  *  0.98  mov edx,eax
  *  0.98  imul edx,eax
  *  0.98  sar edx,8
  *  0.98  mov ecx,ebx
  *  0.98  imul ecx,ebx
  *  0.98  sar ecx,8
  *  0.98  lea edi,[edx+ecx]
  *  0.98  cmp edi,400h
  *  0.98  jle short 00010060h
```
```
## mandel clangO2: header 0x10080, 9449 iterations, 16.45 instructions and 0.94 memory operands per iteration, 93% of 167607
     1.00  imul ecx,ecx
     1.00  shr ecx,8
     1.00  mov ebp,eax
     1.00  imul ebp,eax
     1.00  shr ebp,8
     1.00  lea edx,[ebp+ecx]
     1.00  cmp edx,400h
     1.00  ja short 00010044h
  *  0.94  imul eax,esi
  *  0.94  sar eax,7
  *  0.94  add eax,[esp+0Ch]
  *  0.94  add ecx,ebx
  *  0.94  sub ecx,ebp
  *  0.94  inc edi
  *  0.94  cmp di,20h
  *  0.94  mov esi,ecx
  *  0.94  jne short 00010080h
```

### nbody
```
## nbody llrm: header 0x10184, 1200 iterations, 38.00 instructions and 14.00 memory operands per iteration, 75% of 60677
     1.00  fld st,qword ptr [ebp+esi*8-44h]
     1.00  fld st,qword ptr [ebp+edx*8-44h]
     1.00  fsubp
     1.00  fld st,qword ptr [ebp+esi*8-64h]
     1.00  fld st,qword ptr [ebp+edx*8-64h]
     1.00  fsubp
     1.00  fxch
     1.00  fld st,st(0)
     1.00  fmul st,st(1)
     1.00  fxch st,st(2)
     1.00  fld st,st(0)
     1.00  fmul st,st(1)
     1.00  faddp st(3),st
     1.00  fxch st,st(2)
     1.00  fadd st,dword ptr [102E4h]
     1.00  fdivr st,qword ptr [102E8h]
     1.00  fld st,qword ptr [ebp+edx*8-84h]
     1.00  fxch st,st(2)
     1.00  fmul st,st(1)
     1.00  fadd st(2),st
     1.00  fxch st,st(2)
     1.00  fstp qword ptr [ebp+edx*8-84h],st
     1.00  fld st,qword ptr [ebp+edx*8-0A4h]
     1.00  fxch st,st(3)
     1.00  fmulp
     1.00  fadd st(2),st
     1.00  fxch st,st(2)
     1.00  fstp qword ptr [ebp+edx*8-0A4h],st
     1.00  fld st,qword ptr [ebp+esi*8-84h]
     1.00  fsubrp
     1.00  fstp qword ptr [ebp+esi*8-84h],st
     1.00  fld st,qword ptr [ebp+esi*8-0A4h]
     1.00  fsubrp
     1.00  fstp qword ptr [ebp+esi*8-0A4h],st
     1.00  inc bx
     1.00  movzx esi,bx
     1.00  cmp bx,4
     1.00  jb 00010184h
```
```
## nbody gccO2: header 0x100c8, 1200 iterations, 27.00 instructions and 11.00 memory operands per iteration, 68% of 47851
     1.00  fld st,qword ptr [esp+eax*8+8]
     1.00  fsub st,st(2)
     1.00  fld st,qword ptr [esp+eax*8+28h]
     1.00  fsub st,st(2)
     1.00  fld st,st(1)
     1.00  fmul st,st(2)
     1.00  fld st,st(1)
     1.00  fmul st,st(2)
     1.00  faddp
     1.00  fadd st,dword ptr [11018h]
     1.00  fdivr st,st(5)
     1.00  fmul st(2),st
     1.00  fld st,qword ptr [ecx]
     1.00  fadd st,st(3)
     1.00  fstp qword ptr [ecx],st
     1.00  fmulp
     1.00  fld st,qword ptr [edx]
     1.00  fadd st,st(1)
     1.00  fstp qword ptr [edx],st
     1.00  fxch
     1.00  fsubr st,qword ptr [esp+eax*8+48h]
     1.00  fstp qword ptr [esp+eax*8+48h],st
     1.00  fsubr st,qword ptr [esp+eax*8+68h]
     1.00  fstp qword ptr [esp+eax*8+68h],st
     1.00  inc eax
     1.00  cmp ax,4
     1.00  jne short 000100C8h
```
```
## nbody clangO2: header 0x10220, 1200 iterations, 27.00 instructions and 12.00 memory operands per iteration, 62% of 52070
     1.00  fld st,st(1)
     1.00  fsubr st,qword ptr [esp+esi*8+78h]
     1.00  fld st,st(1)
     1.00  fsubr st,qword ptr [esp+esi*8+58h]
     1.00  fld st,st(0)
     1.00  fmul st,st(1)
     1.00  fld st,st(2)
     1.00  fmul st,st(3)
     1.00  faddp
     1.00  fadd st,dword ptr [11000h]
     1.00  fdivr st,qword ptr [11008h]
     1.00  fmul st(2),st
     1.00  fld st,st(2)
     1.00  fadd st,qword ptr [esp+edx*8+28h]
     1.00  fstp qword ptr [esp+edx*8+28h],st
     1.00  fmulp
     1.00  fld st,st(0)
     1.00  fadd st,qword ptr [esp+edx*8+8]
     1.00  fstp qword ptr [esp+edx*8+8],st
     1.00  fxch
     1.00  fsubr st,qword ptr [esp+esi*8+30h]
     1.00  fstp qword ptr [esp+esi*8+30h],st
     1.00  fsubr st,qword ptr [esp+esi*8+10h]
     1.00  fstp qword ptr [esp+esi*8+10h],st
     1.00  inc esi
     1.00  cmp esi,3
     1.00  jne short 00010220h
```

### crc
```
## crc llrm: header 0x1000f, 9 iterations, 52.00 instructions and 1.00 memory operands per iteration, 98% of 478
     1.00  movzx edx,byte ptr [ecx+100D9h]
     1.00  xor eax,edx
     1.00  mov edx,eax
     1.00  shr edx,1
     1.00  and eax,1
     1.00  neg eax
     1.00  and eax,0EDB88320h
     1.00  xor edx,eax
     1.00  mov ebx,edx
     1.00  shr ebx,1
     1.00  and edx,1
     1.00  neg edx
     1.00  and edx,0EDB88320h
     1.00  xor ebx,edx
     1.00  mov edx,ebx
     1.00  shr edx,1
     1.00  and ebx,1
     1.00  neg ebx
     1.00  and ebx,0EDB88320h
     1.00  xor edx,ebx
     1.00  mov ebx,edx
     1.00  shr ebx,1
     1.00  and edx,1
     1.00  neg edx
     1.00  and edx,0EDB88320h
     1.00  xor ebx,edx
     1.00  mov edx,ebx
     1.00  shr edx,1
     1.00  and ebx,1
     1.00  neg ebx
     1.00  and ebx,0EDB88320h
     1.00  xor edx,ebx
     1.00  mov ebx,edx
     1.00  shr ebx,1
     1.00  and edx,1
     1.00  neg edx
     1.00  and edx,0EDB88320h
     1.00  xor ebx,edx
     1.00  mov edx,ebx
  ... (13 more lines)
```
```
## crc gccO2: header 0x10050, 72 iterations, 8.00 instructions and 0.00 memory operands per iteration, 88% of 656
     1.00  mov ecx,eax
     1.00  shr ecx,1
     1.00  and eax,1
     1.00  neg eax
     1.00  and eax,0EDB88320h
     1.00  xor eax,ecx
     1.00  dec dx
     1.00  jne short 00010050h
```
```
## crc clangO2: header 0x10010, 9 iterations, 47.00 instructions and 1.00 memory operands per iteration, 97% of 436
     1.00  movzx esi,byte ptr [ecx+11009h]
     1.00  xor esi,eax
     1.00  mov eax,esi
     1.00  shr eax,1
     1.00  mov edx,esi
     1.00  and edx,1
     1.00  neg edx
     1.00  and edx,0EDB88320h
     1.00  xor edx,eax
     1.00  mov edi,edx
     1.00  shr edi,1
     1.00  mov eax,esi
     1.00  shl eax,1Eh
     1.00  sar eax,1Fh
     1.00  and eax,0EDB88320h
     1.00  xor eax,edi
     1.00  mov edi,eax
     1.00  shr edi,6
     1.00  mov ebp,esi
     1.00  shl ebp,1Dh
     1.00  sar ebp,1Fh
     1.00  and ebp,76DC419h
     1.00  mov ebx,esi
     1.00  shl ebx,1Ch
     1.00  sar ebx,1Fh
     1.00  and ebx,0EDB8832h
     1.00  xor ebx,ebp
     1.00  mov ebp,esi
     1.00  shl ebp,1Bh
     1.00  sar ebp,1Fh
     1.00  and ebp,1DB71064h
     1.00  xor ebp,ebx
     1.00  shl esi,1Ah
     1.00  sar esi,1Fh
     1.00  and esi,3B6E20C8h
     1.00  xor esi,ebp
     1.00  xor esi,edi
     1.00  shl edx,1Ah
     1.00  sar edx,1Fh
  ... (8 more lines)
```

### fib (private function: llrm `-S`, then gcc -O2 `objdump`, abridged)

```
push ebp
    mov ebp, esp
    sub esp, 8
L0_0:
    mov eax, dword ptr [ebp+8]
    cmp eax, 2
    jl L0_9
L0_2:
    mov ecx, eax
    dec ecx
    push ecx
    mov dword ptr [ebp-4], eax
    call _fib
    mov dword ptr [ebp-8], eax
    mov eax, dword ptr [ebp-4]
    sub eax, 2
    push eax
    call _fib
    mov ecx, eax
    mov eax, dword ptr [ebp-8]
    add eax, ecx
L0_9:
    leave
    ret 4
```

```
fib:                                  ; argument in eax, no frame
    push edi / push esi / push ebx
    mov  esi,eax
    dec  eax
    jle  done
    mov  ebx,esi
    xor  edi,edi
 loop:
    lea  eax,[ebx-1]
    call fib                          ; fib(n-1)
    sub  ebx,2                        ; fib(n-2) becomes the loop
    add  edi,eax
    cmp  ebx,1
    jg   loop
    and  esi,1
    lea  eax,[esi+edi]
    pop ebx / pop esi / pop edi / ret
```
