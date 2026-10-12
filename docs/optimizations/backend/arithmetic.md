> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/arithmetic.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/arithmetic.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/arithmetic.rs sha256:425e07029a2dbaedde7fad1ee7845311acca72e11cb19a9d772ac6aafb9c7a9b -->

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

## Used by
`isel/matcher.rs` (which multiply to select), `division.rs` (a divisor's
multiply chain), and the QB ABI (`abi/qb.rs`) for its multiply costs.
Design notes: `docs/machine/arithmetic-targets.md`.

## See also
GCC's `synth_mult`:
<https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/expmed.cc>
