> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/upperzero.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/upperzero.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/upperzero.rs sha256:c8b4905b5d60f4078f4f4c852a8ec77e466ed1df69dc10687e68cd1170da3a3b -->

# upperzero

Which 32-bit register roots hold zero in their upper half, and the `movzx`
that makes it so where a 32-bit address needs it.

## How it works
In 16-bit code a 32-bit effective address through a register is the prefixed
form of the 16-bit one. It names the same byte only if the register's upper
half is zero. A forward dataflow over the allocated body keeps, before each
instruction, a set of roots (one bit each) known to be zero above bit 15.

```text
 entry            nothing known (empty set)
 meet             intersection over predecessors; unreached blocks are top
 movzx r32,r/m    sets r       xor r,r / sub r,r        sets r
 mov r32,imm      sets r when imm is 0..0xFFFF and has no relocation
 and r32,imm      sets r when imm is 0..0xFFFF and the source is r
 mov a,b          a is zero when b is
 xchg a,b         swaps the two bits
 word/byte write  keeps the bit; any write of lanes 2 or 3 clears it
 unknown effects  clear every bit (jumps and branches write nothing)
```

`established` makes the facts true where the code needs them. A cell that
reads a root 32 bits wide, whether the register holds a value or not (the
frame pointer), needs that root's upper half zero. Where the analysis does
not prove it, the pass inserts `movzx r32,r16`: in the preheader of the
outermost loop around the cell in which nothing disturbs the root, when the
loop's entry is one block with one successor, or else before the first such
cell in the block. It repeats until no cell lacks the fact.

## Example
`mov ecx, 2; xchg esi, ecx; nop` in one block, nothing known on entry.

Schematic, from the fixture of test
`test_an_exchange_swaps_the_zero_upper_halves` (not printer output):
```lir
mov ecx, 2          ; ecx is zero above bit 15
xchg esi, ecx       ; the two bits swap: esi zero, ecx not
nop                 ; before here: esi in the set, ecx not
```

The exchange moves the zero upper half from `ecx` to `esi`, so a `movzx` of
`esi` before it indexes would be redundant.

From unit test `test_an_exchange_swaps_the_zero_upper_halves` in
`upperzero.rs`.

`hoisted` (m32 and other targets with the rule `zero_extensions`) puts the
same `movzx` in front of a loop for a word register whose widening `movzx`
the loop repeats, where the loop writes the register's low word only: the
widening is then a copy (`peephole.peep`, `zero_extended_copy`). The frame
register is zeroed when the body writes it (no frame pointer: its cells are
esp-relative later) and no instruction reads it whole by name.

## Provides
`before`, the zero roots before each instruction, keyed by instruction
identity. The answer is kept between asks and brought up to date from the
blocks that changed ([`crate::backend::incremental`](incremental.md));
`LLRM_CHECK_ZEROROOTS=1` solves the body from scratch after every answer and
asserts they agree. `preheaded` places `movzx` in loop preheaders for roots
a caller asks for.

## Runs
`established` is the last sub-pass of [`crate::backend::peephole`](peephole.md), run when
the body has such a cell (`needs::UNHELD`) and not on a native target, whose
addresses read whole registers.
`hoisted` runs before the zero-extension rules of the same phase.

## Used by
[`crate::backend::exactaddress`](exactaddress.md) (`before`, `preheaded`), and the rewrite
guards of the peephole (`upper_zero`).
