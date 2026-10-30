> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/valuetrack.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/valuetrack.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/valuetrack.rs sha256:0e4aacd5fcc6c4aa0a8064731ee8b211ed83cd9b88af1a55875865a76e3be388 -->

# valuetrack

Where each `-g` variable's value is over the emitted code.

## How it works
Instruction selection says which instructions define a value a variable
names, and masm marks where each landed: `Mark::Def` (the register or frame
cell the instruction before the mark wrote) and `Mark::Note` (a variable
takes a value). Nothing else of the allocator is read.

The emitted bytes are decoded and followed from the entry along every path,
with what each register and frame cell holds. A move of a whole register or
cell copies what it holds, any other write loses it, a call loses what the
callee clobbers, and a push loses the cells it covers. Where paths join,
only what all of them agree on is kept. If the function lets a frame cell's
address out, a call or a write through a pointer may write any cell, so
those lose every cell.

```text
 code:   mov [bp-4],ax        ; Def: value 7 is in [bp-4]
         mov bx,ax            ; bx holds 7 too
         call f               ; bx lost; [bp-4] kept unless it escaped
 note:   x takes value 7      ; x is in [bp-4]
```

A variable is where its value is: a register if one holds it, else a cell,
or a constant. No value is the answer wherever the code cannot be followed
or the value is in no register or cell. An optimizer that wrote the value
somewhere unrecorded costs the variable its location there, never a wrong
one.

## Example
Value 7 is defined in `eax` at offset 8 and tracked over this code.

Schematic, from the fixture of test
`a_variable_follows_its_value_from_the_register_to_the_cell_a_move_copied_it_to`
(not printer output):
```text
 0  push ebp
 1  mov ebp, esp
 3  mov eax, 5
 8  mov [ebp-4], eax      ; Def: value 7 in eax; Note: x takes value 7
11  add eax, 1            ; eax now holds something else
14  ret
```

```text
variable x   from  to   place
             8     14   eax
             14    15   [ebp-4] (4 bytes)
```

The variable moves to the cell the `mov` copied it to once `eax` is
rewritten.

From unit test
`a_variable_follows_its_value_from_the_register_to_the_cell_a_move_copied_it_to`
in `valuetrack_tests.rs`.

## Runs
On the emitted code of a procedure, from `debuginfo`, when `-g` places
variables. A variable any of whose notes was not emitted gets no location.

## See also
LLVM's instruction-referencing
[`LiveDebugValues`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/LiveDebugValues/InstrRefBasedImpl.cpp).
