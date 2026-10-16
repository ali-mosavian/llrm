> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/masm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/masm.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/masm.rs sha256:492a015be48418d40b788581597cb017b3f3245286f666e0cbfafbf50b2df530 -->

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
                                                block that first needs
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

A frame piece (the frame, one saved register) is set up where it is first
needed rather than at the entry when `shrinkwrap::wrapped` says so, and is
taken back on the returns that path reaches.

`cleaned_returns` rewrites each `ret` of a callee that removes its
arguments to `ret n` or `retf n`; past what `ret imm16` can remove, a
near return of a 32-bit target pops the return address into the last slot
of the area instead (`moved_return`). `return_overhead_bytes` is the size of
the leave sequence, which `jumps::duplicated_returns` needs.

## Example
The allocated LIR has no frame sequences. `listing` adds the `push` and
`pop` of the callee-saved `si` the body uses (the `push` in `B1`, where `si`
is first written), a `ret` after each leave, and the labels.

Before (`ControlFlow`):
```lir
B0  entry  → B1 B5
    #39  bx         = mov.i16     ax
     #2  ax         = load.i16    [ss:t1@bx]
     #3  bx, es     = les.i16     [ss:t1@bx+0x4]:i32
     #4               elided
     #6               test.i16    ax, ax
     #7               je          B5 else B1                            ; 3% taken

B1  ← B0  → B4 B2
    #21  ax         = add.i16     ax, ax
    #38  si         = mov.i16     ax
    ; ...
    #11               cmp.i16     cx, 4
    #12               je          B4 else B2                            ; 3% taken
; ...
B4  ← B1 B2 B3  → exit
    #41  sum.1.lcssa1 = elided      sum.1.lcssa1
    #16               ret         sum.1.lcssa1@eax
```

After:
```asm
_total proc near
L0_0:
    mov bx, ax
    mov ax, word ptr ss:[bx]
    les bx, dword ptr ss:[bx+4]
    test ax, ax
    je L0_4294967297
L0_19:
    push si
    add ax, ax
    mov si, ax
    ; ...
    cmp cx, 4
    je L0_13
    ; ...
L0_13:
    pop si
    ret
```

From `ISEL_DUMP=1` and `-S` of a `total` over a `&[u16]` with `llrm-nib -m16
-O2 -fno-inline-functions -fno-inline-functions-called-once`.

## Runs
Once per procedure after the machine phases. `cleaned_returns` runs in
`assemble::assembled_by` (`masm cleaned returns`), before the return
duplication and shrink-wrap tails there; `listing` runs when the object is
built or `-S` is printed.

## See also
[`crate::backend::objbuild`](objbuild.md) for the encoder of the same items,
[`crate::backend::shrinkwrap`](shrinkwrap.md), [`crate::backend::stackusage`](stackusage.md).
