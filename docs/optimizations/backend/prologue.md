> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/prologue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/prologue.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/prologue.rs sha256:e12b7f236a7525e1bc6cc573e4b626e608888cb3c57e0bbe4aa33a31d8fa7ae9 -->

# prologue

Reserves the stack the spiller asked for (`sub sp, N`) and gives it back
(`add sp, N`) at every exit.

## How it works
Spill slots and other frame growth are only known after allocation, so the
frame is grown here, by `Frame::size()` bytes. Nothing happens when that is
zero.

Where the code goes depends on the frame plan:

- A runtime-framed procedure (BC): `sub sp, N` goes right after the `B$ENRA`
  call that establishes BP, and `add sp, N` right before each `B$EXSA` call
  that tears the frame down.
- A plain procedure: `sub sp, N` goes in front of the entry block's first
  instruction and `add sp, N` before each return.
- A native frame plan (`frame.native`): the instructions are placed at the
  anchors the plan names (`reserve_at`, `releases`), each of which must be
  one contiguous run of instructions covering source bytes. Outgoing stack
  arguments are rebased first, since sp moves by `N`.

```text
 B$ENRA                       B$ENRA
 body              ==>        sub sp, N        (covers no source bytes)
 B$EXSA                       body
                              add sp, N
                              B$EXSA
```

The new instructions are marked `frame_adjust`, which the peephole drops if
no spill cell remains. The reservation is refused (`Refused`) when no exit
can be found: a body with no return, no `B$EXSA` and no call to the
runtime's exit (`B$CEND`, `B$CENP`), unless it is marked `noreturn`; or when
the entry block is empty; or when an anchor was lost or a native plan is
combined with a runtime entry. Where the body ends the program, `sp` is
lowered and never put back.

## Example
The `procedure` fixture, a runtime-framed body (`call` at 0x0001 is
`B$ENRA`, at 0x0002 `B$EXSA`), with one two-byte spill slot:

```lir
=== before
  f: 4 instructions
    block 0x0000
      0x0000  v1 := mov 0x6
      0x0001  call v1  ; requires v1@CX
      0x0002  call
      0x0003  retf
=== after
  f: 6 instructions
    block 0x0000
      0x0000  v1 := mov 0x6
      0x0001  call v1  ; requires v1@CX
      0x0002  SP := sub SP, 0x2  ; inserted; frame adjust
      0x0002  SP := add SP, 0x2  ; inserted; frame adjust
      0x0002  call
      0x0003  retf
```

`sub sp, 2` follows the entry call and `add sp, 2` precedes the exit call.

From unit test `test_spill_reservation_is_inside_the_runtime_frame` in
`prologue.rs`.

## Runs
A machine phase after `parcopy` and before `peephole`, in every machine
(`flow::machine_with`). The frame is the one shared with the spiller and
allocator, so it is final here.

## Needs
The shared [`crate::backend::frame`](frame.md) and the table of called runtime names
(`calls`).
