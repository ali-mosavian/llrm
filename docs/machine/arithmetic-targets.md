# CPU-dependent constant arithmetic

For contributors to the backend's constant arithmetic. A multiply or divide by a constant is lowered during instruction selection by comparing sequences the target prices; the answer depends on `-march` and `-mtune`, never on MIR.

## Selecting the CPU

`-march=CPU` and `-mtune=CPU` take gcc's names (`i386`, `i486`, `pentium`, `pentiumpro`, `k5`, `k6`, `athlon`, `core2`), each a column of the target's `timings.times`; the default is the 486 (`default_cpu`). Tuning changes which instructions are selected, not MIR semantics or the minimum ISA: the code stays 386-compatible. `-S` shows the result.

## Multiply by a constant

`arithmetic::scale` ([`arithmetic.rs`](../optimizations/backend/arithmetic.md)) compares binary and signed-digit chains of shifts, adds, subtracts and `lea` against `imul` by the immediate, each priced from the CPU's column, including the source-preserving move a destructive two-address form needs. Ties keep the multiply. A signed digit chain makes seven `(x << 3) - x`. A negative number is the chain of its magnitude and a `neg`, or, as gcc's `synth_mult` has it, `source - (source * q << m)`. A narrow value gets shifts, adds and subtracts only (`cheapest_narrow_chain`), since `lea` makes a dword. The chains are remembered per profile. On the 386 an `imul` by a positive imm8 costs `max(ceil(log2(m)), 3) + 6` (Intel 80386 Programmer's Reference); other CPUs price it by the immediate's bits from the table.

```
 x * 20 on the 486/P6:   lea bx,[ecx+ecx*4]      x*5
                         shl bx,2                 x*20      (chain)
 not                     imul bx,bx,20            (a multiply the table prices higher)
```

## Divide by a constant

A multiply by a constant that only an address reads is no multiply when the constant is 3, 5 or 9 times an address scale (1, 2, 4 or 8): `a[3*i]` over `int` is `lea t,[i+i*2]` and `[a+t*4]`, `p[12*i]` over `char` the same. The `lea` is as many bytes as `imul r,r,imm8` and one clock, so the rule holds at every level, -Os included; the scale is the address's, not a `shl`. Any other constant a GEP index is multiplied by is priced as the patterns price it (`cheaper_chain`: a chain below the `imul` in the profile's channel, bytes then clocks tuned for size), where the index had been an `imul` whatever the table said. A pointer something else than an access reads keeps the old multiply.

`division::reciprocal` ([`division.rs`](../optimizations/backend/division.md)) replaces a signed dword division by a constant divisor with a multiply by a magic number (LLVM's `SignedDivisionByConstantInfo`, `magic`) when the CPU's multiply costs less than its divide. Negative divisors divide by the magnitude and negate; zero, `-1`, `INT_MIN` and word divisions keep `idiv`. The cost of the reciprocal is the multiply's clocks for the magic's bits, the shifts and corrections, and the reconstruction of the remainder (a `scale` chain, or an `imul`), counting each dword operand's prefix decode (`prefix` in `timings.times`); `timing.rs` holds audited core-clock ranges per form and width, and a CPU with no entry keeps `idiv`. When only the quotient is used, the reconstruction is left out of the estimate and the code. At -Os the division stays: the magic is a 5-byte `mov` before the multiply and corrections, where `cdq; idiv` is 3 bytes.

```
 x / 7, multiply cheaper            x / 7, divide cheaper
 mov ecx,92492493h                   mov ebx,7
 mov eax,ebx                         mov eax,ecx
 imul ecx                            cdq
 mov eax,edx ; add eax,ebx           idiv ebx
 mov ecx,eax ; shr ecx,31
 sar eax,2   ; add eax,ecx           (quotient only: no remainder tail)
```

The prefix rule is Intel 241430-004 section 24.3 (page 24-3).

## See it

`llrm-cc -march=pentium -S` prints the selection for a CPU (compare `-march=i486`); `arithmetic.rs` and `division.rs` tests pin the chosen sequences. These are static estimates; [metal](metal.md) is about what only hardware can measure.
