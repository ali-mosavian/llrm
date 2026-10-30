> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/inline_asm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/inline_asm.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/inline_asm.rs sha256:1e1c5b8935134a15ffc91c0f3644d17af3055846d328981b3eaa61a8103d0aca -->

# inline_asm

Names the QB runtime's registers for inline assembly operands and clobbers.

## How it works
The assembler is `llrm_x86::asm`, re-exported here (`Mode`, `Refusal`,
`assembled`). What this file adds is the mapping between the register names
a program writes in an inline block and the runtime's `Reg`, the registers a
runtime routine's contract names:

- `operand_register` takes a name an input or output may use (`ax`, `al`,
  `ah`, ..., `si`, `di`: the registers a call argument can be pinned to) and
  gives its 16-bit register and the part named (`Part::Word`, `Low`,
  `High`).
- `clobbered` takes a register a block may declare it changes: those plus
  `es` and `flags`. The rest (`sp`, `bp`, `ds`, `ss`, `cs`) the program
  depends on, and a block restores any it changes.
- `view` resolves a general register name, including 8 and 32-bit views, to
  its root `Reg` and the bits of the view (`eax` is `(Ax, 32)`, `al` is
  `(Ax, 8)`); the names are x86's whatever the target.
- `machine` is the inverse: the machine register for a root and a width.

## Example
No honest example: the module is a table of register names, and the
assembler that turns an inline block into bytes is `llrm_x86::asm`. Read its
tests for inputs and bytes.

## Used by
`abi/qb.rs` and the nib frontend (`llrm-nib` `semantic/assembly.rs`).
