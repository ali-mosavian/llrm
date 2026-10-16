> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/prologue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/prologue.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/prologue.rs sha256:b283a71113dc7b78afb2c12d83c0b8ae6cab7314832f7100ff031eb19ac3de03 -->

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
The `procedure` fixture, a runtime-framed body (`#1` is `B$ENRA`, `#2` is
`B$EXSA`), with one two-byte spill slot:

```lir
fn f()

B0  entry  → exit                                                 ×1.0
     #0  t1         = mov.i16     6
     #1               call        far B$ENRA(t1@cx)
     #2               call        far B$EXSA()
     #3               ret
```

After:

```lir
fn f()

B0  entry  → exit                                                 ×1.0
     #0  t1         = mov.i16     6
     #1               call        far B$ENRA(t1@cx)
     #4  sp         = sub.i16     sp, 2                                  ; frame
     #5  sp         = add.i16     sp, 2                                  ; frame
     #2               call        far B$EXSA()
     #3               ret
```

`sub sp, 2` follows the entry call and `add sp, 2` precedes the exit call.

From unit test `test_spill_reservation_is_inside_the_runtime_frame` in
`prologue.rs`. No dump prints this phase, so the lines are set by hand in
the dump's format.

## Runs
A machine phase after `parcopy` and before `peephole`, in every machine
(`flow::machine_with`). The frame is the one shared with the spiller and
allocator, so it is final here.

## Needs
The shared [`crate::backend::frame`](frame.md) and the table of called runtime names
(`calls`).
