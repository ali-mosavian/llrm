> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/statustest.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/statustest.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/statustest.rs sha256:6052b7419bf714e1ab2d407e43b5125125c81ae9f59d893ef7f49b45287d2bd1 -->

# statustest

An x87 compare read from the status word: `fnstsw ax; test ah, mask; jcc`
for `fnstsw ax; sahf; jcc`. DEVIATE X87-FCMP-AH (owner x87, exit
2026-11-20; no pin pass: the pin's `SAHF` is kept everywhere). gcc's
`ix86_expand_fp_compare` makes the same choice; its code is not translated.

## How it works
After `fucom`, `fnstsw ax` puts C0 in bit 0 of `ah`, C2 in bit 2 and C3 in
bit 6; `sahf` copies those into CF, PF and ZF. A condition on one flag, or
on CF and ZF together (`ja`, `jbe`), is one `test ah, mask` and a `je` or
`jne`: `ja` is `test ah, 0x41; je`. A `sahf` followed by exactly one flag
reader, a `jcc`, `setcc` or `cmovcc` of those conditions with nothing
else reading a flag after it, becomes the test. A `sahf` several readers
share (`==` is `je` and `jnp`) stays.

The test is one clock on a 486 (`test r8, imm`) against two for `sahf`, and
two bytes longer: made where the target prices `sahf` above a register-
immediate ALU operation, and not when optimizing for size.

## Example
`fcomp; fnstsw ax; sahf; jbe L` (`x_ld_abs` -O1, 256 executions):

```text
  fnstsw ax                fnstsw ax
  sahf              =>     test ah, 0x41
  jbe L                    jne L
```

## Runs
After allocation, before the peephole optimizer, on a target whose profile
prices `sahf`.
