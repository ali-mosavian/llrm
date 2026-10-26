> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/masm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/masm.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/masm.rs sha256:534ada3b9cbc764dddb8dfacef991b332ce6c8003745213fe927e9f1b053d958 -->

# masm

The assembly-level module: allocated LIR with its frame sequences written
out, as the items `objbuild` encodes and the jwasm source `-S` prints.

## How it works
`assemble` collects each machined function into a `Procedure` (its LIR
body, the bytes it reserves below the frame register, its callees, flags)
and the module's data into `Datum`s, giving a `Module`. `listing` turns
one procedure into `Item`s, and `text` prints the module from them. The
printer and the encoder read the same items, so the two cannot drift. Every
operand is already placed; an unplaced one is an error (`Unprintable`).

A procedure that makes calls and needs the stack aligned (`calleefacts`, the
target's `stack_alignment`) reserves what makes the return address, its
pushes, the frame register and the reserve a multiple of it, so a call finds
the stack aligned; a call's arguments are padded to it by `isel`.

```text
  Procedure.body (LIR)     frame facts          Item list
  ----------------------   ------------------   -----------------------
  blocks, returns       -> parts(): entry   ->  enter (entry only)
  cleaned_returns          and leave            block labels, items
  (ret imm / retf n)       sequences            leave before each ret
                           shrinkwrap::wrapped  pieces set up in the
                                                the Save block
```

The entry and return sequences are not in the LIR. `parts` builds them:
`push bp; mov bp,sp; sub sp,reserve` (or `enter` where size is tuned and
the target has it), the stack-limit compare and its overflow branches, a
`push` per callee-saved register the body names or a call disturbs, and
the mirror on return. An interrupt handler wraps its frame in a save of
every register and the segment setup, and a runtime-built frame is not
rebuilt.

Where the frame register is not needed (nothing holds a value in it), the
frame is dropped and each cell is addressed through the stack pointer
(`stack_addressed`): every instruction's stack effect moves the offset,
and every way into a block (a branch, or the block before it falling through
where the next block is among its successors) must agree on it. Anything
that puts that out of reach (the stack pointer written another way, inline
code other than x87 register operations, an indirect call that may pop, the
frame register read as a value) keeps the frame. A BASIC procedure's
compiler-built frame is like any other; one the runtime builds (`B$ENRA`)
is walked by the runtime and kept. Tuned for size, the stack-addressed form
is kept only if it is no longer in bytes.

Under `-g` the items also carry marks (line starts, body start and end,
where a variable's value is defined) which encode to no bytes. A move of an
immediate into a segment register, which x86 lacks, is written `push imm;
pop seg`.

The frame and the saved registers are set up at the Save block rather than
the entry when `shrinkwrap::wrapped` says so, and are taken back before the
return of the Restore block.

`cleaned_returns` rewrites each `ret` of a callee that removes its
arguments to `ret n` or `retf n`; past what `ret imm16` can remove, a
near return of a 32-bit target pops the return address into the last slot
of the area instead (`moved_return`). `return_overhead_bytes` is the size of
the leave sequence, which `jumps::duplicated_returns` needs.

## Example
The allocated LIR has no frame sequences. `listing` adds the `push` and
`pop` of the callee-saved `si` the body uses, a `ret` after each leave, and
the labels.

Before:
```lir
=== ControlFlow
  total: 25 instructions
    block 0x0000
      0x0000     ; inserted; delivers v19@AX
      0x0000  SI := mov AX  ; inserted
      ; ...
      0x0003  cmp DX, 0x4  ; inserted
      0x0003  jne   ; inserted
      0x0003     ; inserted
      0x0003     ; inserted
      0x0003     ; inserted; requires v20@EAX
    block 0x0009
      0x000a  cmp DX, CX  ; inserted
      0x000a  jae   ; inserted
    ; ...
```

After:
```asm
_total proc near
    push si
L0_0:
    mov si, ax
    ; ...
    cmp dx, 4
    jne L0_9
    pop si
    ret
L0_9:
    cmp dx, cx
    jae L0_18
    ; ...
```

From `ISEL_DUMP=1` and `-S` of a `total` over a `&[u16]`
(`-fno-inline-functions`) at `-m16 -O2`.

## Runs
Once per procedure after the machine phases. `cleaned_returns` runs in
`assemble::assembled_by` (`masm cleaned returns`), before the return
duplication there; `listing` runs when the object is
built or `-S` is printed.

## See also
[`crate::backend::objbuild`](objbuild.md) for the encoder of the same items,
[`crate::backend::shrinkwrap`](shrinkwrap.md), [`crate::backend::stackusage`](stackusage.md).
