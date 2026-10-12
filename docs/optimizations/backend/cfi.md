> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/cfi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/cfi.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/cfi.rs sha256:0f961743b75385aa14bf5fd0da6fd6a7916480f7c47342892eb877a867c4a09d -->

# cfi

Reads call frame information back from emitted code.

## How it works
For every offset of a function the rows say how to find the caller's
frame (the canonical frame address, CFA) and where each saved register
lives. They are derived from the bytes, not from what the compiler meant,
so they hold for any producer.

`rows` decodes from the entry and follows every path with a work list.
Each place carries a `State`: whether the CFA is measured from the stack
pointer or the frame register, the distance, the registers saved (and
their depth below the CFA) and the registers written since entry. `step`
updates it per instruction: push and pop move the stack and record a save
of an unchanged register or its restore, `sub`/`add` of the stack
pointer by an immediate moves it, `mov frame,stack` switches the base to
the frame register, `mov stack,frame` and `leave` switch back, a call
subtracts the argument bytes the callee pops (`pops`). Rows that repeat
the previous one are merged.

```text
 push ebp           CFA = esp+4   ->  esp+8, ebp saved at CFA-8
 mov  ebp,esp       CFA = esp+8   ->  ebp+8
 sub  esp,8         CFA = ebp+8
 leave              CFA = ebp+8   ->  esp+4, ebp restored
 ret                CFA = esp+4
```

A rule that is not single-valued is an error, and the caller keeps no
frame information for that function: two paths reaching a place with
different stack depths, a write to the stack pointer by anything but the
forms above, a write to the frame register while it is the base, invalid
code. A jump out of the function (a tail call) ends that path. Code no path
reaches (jump table targets) gets no row and takes the rule of the code
before it.

## Used by
`debuginfo.rs`, under `-g`.

## See also
`arrival.rs`, which reads the same code for where a parameter is.
