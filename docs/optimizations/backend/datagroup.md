> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/datagroup.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/datagroup.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/datagroup.rs sha256:89d440bed7e25a38c8825831037ad4587f17f156c27a4e0b905857ca62029f5c -->

# datagroup

Make the data segment register a fourth selector register where the
stack lives in the data group.

## How it works
When the stack segment reaches the same memory as the data segment, an
access to the data group can name the stack segment instead (`ss:`).
The data segment register is then free to hold a far-pointer selector
between the points that need it to hold the data group
(`target::needs_data_group`: calls, returns, traps, opaque
instructions, x87 and string instructions). Allocation may place a
selector there. This pass makes the rest of the body agree.

`restored` runs a forward analysis, "the register may hold something
else", over the allocated body. A write to the register sets it. It
stays set across blocks to every successor.

```text
  mov  ds, bx            ; DS now a selector       dirty
  mov  ax, [si]          ; a data cell      =>     mov ax, ss:[si]
  les  di, [var]         ; data cell        =>     les di, ss:[var]
  call f                 ; needs DS = data  =>     mov ds, ss   (restore)
                                                   call f
```

Where it is set, each memory operand that would take its segment from
the data segment register is prefixed with the stack segment. That means
an operand with no segment of its own, in a segment, external or
literal space, whose base is not itself the frame or stack register
(those select the stack segment already). Immediately before an
instruction that needs the data group the register is reloaded from the
stack segment register and the flag clears. A block with no successor
that leaves with the flag set restores before its jump or return.

Nothing is done when the target has no stack-segment route
(`Segments::through` is none) or when the body writes no data segment
register. `names_data_segment` tells allocation to leave the register
alone when the body already names it, since such a body manages DS
itself, and then `data_free` is false here.

The runtime's interrupt handlers load their own data segment, so a value
held in DS is safe from them.

## Example
No honest example: the file has no unit test, and a small program that makes
the allocator put a selector in `ds` under `-mstack-is-data` was not found.

## Used by
Register allocation (`allocate.rs`): `names_data_segment` before it,
`restored` on the allocated body it keeps.

## See also
`-m[no-]stack-is-data`; `crate::backend`
