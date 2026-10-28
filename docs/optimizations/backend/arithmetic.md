> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/arithmetic.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/arithmetic.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/arithmetic.rs sha256:81aa4576d6c371b95545715d03e38fd6e67ee44d6d137d042d13044570dbc3dc -->

# arithmetic

Ranks the ways to multiply by a constant on a target: shift and add
chains, `lea`, or `imul`.

## How it works
A constant multiply is priced two ways. `immediate_multiply` is the
clocks of `imul r,r,imm` on the CPU profile: on a 386 the manual's
`max(ceil(log2 m), 3) + 6` for a positive imm8, elsewhere the timing
table's range by the multiplier's bits, else the flat `imul_r32` price.

`cheapest_chain` is the cheapest sequence of `shl`, `add`, `sub`, `lea`
and copies. Horner chains over binary and signed digits give a first
bound. Then `synth` searches as GCC's `synth_mult` does: an even `t` is
a shift of `t >> m`; an odd `t` is `t-1` or `t+1` plus or minus the
source; a factor `2^m +- 1` is `cur + cur<<m` or `cur<<m - cur`, one
`lea` for a scale of 2, 4 or 8. The search stops at the best cost so far
and remembers what it found per `t`. A negative multiplier is the chain
of its magnitude and a `neg`, or of `1 - n` shifted and subtracted from
the source. `cheapest_narrow_chain` drops `lea` for values narrower than
a dword.

```text
 x * 5, no lea             x * 5, with lea
   mov  t,x                  lea  t,[x + x*4]
   shl  t,2
   add  t,x
```

`scale` returns the chain only where it is cheaper than `imul`.
Results are kept in the profile, as a multiply is asked about again and
again.

DEVIATE M1 (AM-M1, priced): the pin's X86 does a scalar constant multiply
by `combineMul`
([X86ISelLowering.cpp#L51191](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L51191))
and `decomposeMulByConstant` (#L3636): a `lea` for 3, 5 and 9 (times a power
of two) and shifts and adds for 2^n +- 1, otherwise `imul`; ours searches
the CPU profile's clocks (above), so a product like `x*7 + y` in an unrolled
loop is `lea`s where the pin has `imul`. Price, 2026-10-23, one build with
the chains switched off by a probe, 71 vsgcc programs at m32: clocks with
the chains are -5.1% / -5.9% / -6.0% at -O1 / -O2 / -O3, code
+3072 / +720 / +688 B (the chains cost bytes), -Os identical (no chain is
chosen; the DAG path since 2026-10-27 takes them at m32 -Os as the pin does:
+3037 B over 58 objects of tests/run/c, histo and x_dct clocks equal the
pin's, and keeps `imul` at m16 -Os, where they grow 27 of 175 by 3550 B);
QCport m16 -38 B at -O1..-O3, 0 at -Os. Instructions against the pin on the
same IR: lea +471 at -O2 (`docs/working/instruction-gap-vs-pin.md`). Exit:
none while the clocks decide; reviewed with the tuning phase. Owner w-4,
review 2026-12-01.

## Example
What the ranking answers on a 486 (`targets::tuned_for(M32, "486")`):

```text
cheapest_chain(3, 5, 9)          one lea each: [x+x*2], [x+x*4], [x+x*8]
cheapest_narrow_chain(-15)       shl 4 ; rsub        (x - x*16, no lea in a word)
immediate_multiply, clocks       3 -> 13   10 -> 14   -3 -> 15   1103515245 -> 41
```

From unit tests `test_a_factor_of_one_more_than_a_scale_is_one_lea`,
`test_synthesized_chains_are_the_product_and_shorter_than_horner` and
`test_a_multiply_by_a_wide_constant_is_priced_by_its_multiplier_on_a_486` in
`arithmetic.rs`.

## Used by
`isel/matcher.rs` (which multiply to select), `division.rs` (a divisor's
multiply chain), and the QB ABI (`abi/qb.rs`) for its multiply costs.
Design notes: `docs/machine/arithmetic-targets.md`.

## See also
GCC's `synth_mult`:
<https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/expmed.cc>
