> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/demandedbits.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/demandedbits.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/demandedbits.rs sha256:0f2012810eb212cb6ec1422f942b6747bc967644821fd4912b67b10012b511eb -->

# demandedbits

Which bits of each integer value anything reads: a translation of LLVM's
`DemandedBits` (`DemandedBits.cpp` at d1106deb71cc).

## How it works
A backward fixed point over the use-def graph. Every instruction that must
stay (a terminator, a landing pad, one with a side effect) is a root: its
integer operands are read in full, and a root that is itself an integer
value reads what its own operands need of nothing. A worklist then asks each
visited instruction, given the bits of its result that are read, which bits
of each operand are (`determine_live_operand_bits`: a table per opcode,
using `compute_known_bits` for `and`, `or`, `add`, `sub` and variable
shifts). Demand only grows and is bounded by the width, so loops settle.

An instruction nothing visits is dead; a use whose operand bits are all
unread is a dead use. Both survive edits that do not touch the ones asked.

```text
  %a = load i32          read: 0x000000ff   (only the low byte)
  %m = and i32 %a, 255   read: 0xffffffff   <- stored in full
  store i32 %m, ptr %p
```

## DEVIATEs
1. Vectors, `bswap`, `bitreverse`, `ctlz`, `cttz`, `fshl`, `fshr`, `clmul`
   and the vector element operations are not MIR constructs: their cases are
   absent, which is what LLVM's `default` does for an operand it has no rule
   for. Of the intrinsics only `{s,u}{min,max}` exists.
2. Integers of more than 128 bits are not tracked (MIR has none).
3. The known-bits queries carry no dominator tree, assumption cache or
   dominating-condition cache (`Query` has them as `None`) until the pass
   manager hands them out: LLVM's answers can be stronger, so a demanded
   mask can be wider than LLVM's, never narrower.

## Example
See the tests in `demandedbits_tests.rs`; `and i32 %a, 255` demands only
the low byte of `%a`.

## Provides
`DemandedBits::of`, then `demanded_bits`, `demanded_bits_of_use`,
`is_instruction_dead` and `is_use_dead`. Consumer: `bdce` in
`llrm-transforms`.
