> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/target.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/target.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/target.rs sha256:3a04cf13a807ad2db6a7af80ecc9de9bb666394fb6c8c314a6b7e390b6a8eaea -->

# target

What the machine has for the register allocator: operand needs, the segment
registers, and the facts the allocator asks of each instruction, so it can
place operands without knowing x86.

## How it works
The module has two halves. The first describes where an operand may live.
`Need` is one register or a set of them, and `Occurrence` names an operand
by side and index, because a requirement belongs to an operand and not to
the register it happens to name (`imul`'s dx:ax is the first and second
destination). `reads` and `writes` list the registers an instruction touches
whether or not it names them: the registers its class requirements fix, and
the addressing class for every base, index and `through` register of a
memory operand. `tied` finds the register a two-address instruction reads
and writes as one. `order` turns a `Need` into the registers to try, most
preferred first (LLVM's `AllocationOrder`): no need gives the allocatable
registers, a need of segment registers the selectors it names, any other
need the allocatable registers it names.

The second half is `Segments`: the selector registers a far-pointer value
may be placed in (every segment register the program model does not reserve,
the far pointer's own first), the register an unprefixed access reads, and,
where the stack lives in the data group, the register that reaches the data
group while DS holds something else. `needs_data_group` says which
instructions need DS to hold the data group (calls, returns, x87 and string
instructions, traps).

The remaining functions answer encoding questions: whether `xchg` takes two
operands, the width `push` and `pop` carry a place at, whether an x87
compare reaches the flags through the status word (`fnstsw`, `sahf`), and
which operands are positions in the x87 stack, which no pass may rename.

## Example
No honest example: the module is tables and queries (`Need`, `order`,
`reads`, `writes`, `Segments`) that change no LIR. Its tests in `target.rs`
show each answer.

## Used by
[`crate::backend::allocate`](allocate.md), `crate::coalescer::pass`,
[`crate::backend::constrain`](constrain.md), [`crate::backend::datagroup`](datagroup.md),
[`crate::backend::regclass`](regclass.md), [`crate::backend::parcopy`](parcopy.md),
`crate::backend::ssaspill` and [`crate::backend::masm`](masm.md) (register names).

## See also
LLVM's
[`TargetRegisterInfo`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/include/llvm/CodeGen/TargetRegisterInfo.h)
and
[`TargetInstrInfo`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/include/llvm/CodeGen/TargetInstrInfo.h).
