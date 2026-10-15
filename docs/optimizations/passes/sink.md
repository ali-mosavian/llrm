> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/sink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/sink.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/sink.rs sha256:8dc1d74421b7943c6d29ae70a67f759df667f356786c43a2cdb7044517a9c51b -->

# sink

Work moved down to the one place that needs it, so the paths that skip
that place no longer run it: gcc's `-ftree-sink` (`pass_sink_code`),
LLVM's `Sink` and `MachineSink`.

## How it works
A load whose every reader lies below one block `T` that its own block `B`
dominates moves to the top of the deepest such `T` where it still gains:
`T` runs no oftener than `B`, in no deeper loop, and some path from `B` to
an exit skips `T`. No reader may lie in a deeper loop than `T` (`hoist`
left it where it is), and nothing on any path from the load to `T` may
write what it reads (`memoryssa::Writers`). An operation (the `gep` of a
sunk load) moves the same way, but only when every instruction that reads
it has moved: alone it may part a flag setter from its compare.

The moves are made on a copy and kept only if together they save more than
disturbing the allocation can cost: the work no longer run on the skipped
paths (each instruction's price at the frequency of `B` less that of `T`)
must exceed one register copy for each execution of the moved work where it
now runs. A 3% skip in sum_three at -m16 -O2 cost an instruction; a skip of
over half is what d_faces has.

The memory accesses and the block frequencies are asked of the manager when
a move first needs them, so a body with nothing that passes the shape of
the function asks for none.

Blocks are taken last first, and each from its end, so what a sunk
instruction read is next in line: the `gep` of a sunk load goes with it.
Blocks and edges are as they were.

```text
 b24:  %t = load ptr, ptr %a          b24:  %c = icmp ...
       %c = icmp ...           ==>          br %c, b28, b26
       br %c, b28, b26                b26:  %t = load ptr, ptr %a
 b26:  ... %t ...                           ... %t ...
```

d_faces's inner loop loaded eight arrays' pointers at its top and stored
the ones it needed after a call to the stack; a face the visibility test
skipped paid for all of them.

## Example
Before, `%v` loaded in `b0` and read only in `b1`, which a negative `%c`
reaches:

```mir
b0:
  %v = load i32, ptr @g
  %cold = icmp slt i32 %c, 0
  br i1 %cold, label %b1, label %b2

b1:
  %r = add i32 %v, 1
  ret i32 %r

b2:
  ret i32 0
```

After, as the test asserts it:

```mir
b0:
  %cold = icmp slt i32 %c, 0
  br i1 %cold, label %b1, label %b2

b1:
  %v = load i32, ptr @g
  %r = add i32 %v, 1
  ret i32 %r

b2:
  ret i32 0
```

From unit test `test_a_load_read_in_one_arm_moves_into_it` in
`sink_tests.rs`.

## Runs
Last of the function passes before `switchconv`, after `addresssink`; off
with `-fno-sink` and at -O0.

## Keeps
Dominators and loops: no block or edge changes.
