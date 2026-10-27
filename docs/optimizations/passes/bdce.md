> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/bdce.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/bdce.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/bdce.rs sha256:068bcf86ea84d48509e801be0f0b24a273d005c2a9c22ad758ef388ed720da0d -->

# bdce

Bit-tracking dead code elimination: a translation of LLVM's BDCE
(`BDCE.cpp` at d1106deb71cc). An instruction whose result bits nobody
reads goes, a sign extension whose extension bits are unread becomes a
zero extension, a mask that clears only unread bits is its operand, and an
operand none of whose bits is read becomes zero.

## How it works
[`llrm_analysis::demandedbits::DemandedBits`](../analyses/demandedbits.md) says, per instruction, which
result bits are read and which uses read nothing. The pass walks the
function once: it queues dead instructions, rewrites the three cases above,
and drops the poison-generating flags of everything downstream of a rewrite
that reads fewer than all bits (`clearAssumptionsOfUsers`), since `nsw`,
`nuw` and `exact` were proved of operands that changed. Queued
instructions are erased at the end.

```text
 %a = add i32 %x, 1
 %m = and i32 %a, 255     store i32 %m   ==>  nothing changes: all of %m is read
 %t = trunc i32 %m to i8
 %e = sext i8 %t to i32
 %w = and i32 %e, 255                    ==>  %e = zext i8 %t to i32
```

## DEVIATEs
1. The demanded-bits analysis has no vectors and no `bswap`, `ctlz` and kin
   (see its header), so BDCE has none either.
2. `dropPoisonGeneratingFlags` sets `ctlz`, `cttz` and `abs`'s
   poison-on-zero operand false; MIR has no such intrinsics.

## Example
Before:
```mir
define i32 @f(i8 %x) {
b0:
  %s = sext i8 %x to i32
  %m = and i32 %s, 255
  ret i32 %m
}
```
After:
```mir
define i32 @f(i8 %x) {
b0:
  %s = zext i8 %x to i32
  %m = and i32 %s, 255
  ret i32 %m
}
```
From unit test
`test_a_sign_extension_nobody_reads_the_top_of_is_a_zero_extension`
in `bdce_tests.rs`.

## Runs
LLVM runs it after SCCP, before ADCE (slot 34 of the pipeline). Not in
llrm's pipeline yet: `llrm-pass bdce FILE` runs it alone.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## See also
[`crate::dead`](dead.md) removes the instructions nothing reads;
[`llrm_analysis::demandedbits`](../analyses/demandedbits.md) is the analysis.
