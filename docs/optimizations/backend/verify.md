> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/verify.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/verify.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/verify.rs sha256:8c1dccac6257c3e4ef76e1b6877bb7f73513f08394b3e86a1454b19655911d86 -->

# verify

Checks that a lowered body is well formed, between machine phases.

## How it works
`verify` returns the complaints as sentences; empty is well formed. It
checks:

- Blocks: the entry and every successor exist, and every block is reachable,
  except unreachable blocks that only hold byte ownership.
- Spans: no two instructions claim the same original byte, and no span runs
  backwards.
- Operands: every register named exists on the target at the width it is.
- Values: each is defined before it is read, once in SSA, no phi survives
  phi elimination, and a value read is defined or an explicit body input.
- Slots: once instruction selection tagged them (`slotted`), every frame
  cell names its slot.
- Frequencies: the block frequency table has entries for existing blocks and
  runs no block more often than its predecessors send
  (`Frequency::violations`).

`verify_for` adds, for a target with one address space and no segments, that
no operand is a segment register, no cell has a selector or an override
other than the stack's or data's, and no base or index register is narrower
than the target's addresses.

## Example
A body whose only instruction, `v2 := mov v1`, reads a value nothing
defines.

Schematic, from the fixture of test
`test_a_read_value_must_be_defined_or_an_explicit_body_input` (not printer
output):
```lir
v2 := mov v1
```

```text
value#1 is read but never defined or supplied
```

From unit test `test_a_read_value_must_be_defined_or_an_explicit_body_input`
in `verify.rs`.

## Runs
After instruction selection and after every machine phase when `LLRM_VERIFY`
is set to anything but `0` (`flow::verified`, `flow::checked`). Tests, the
gate and torture set it. A failure names the phase.

## See also
LLVM's
[`MachineVerifier`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/MachineVerifier.cpp).
