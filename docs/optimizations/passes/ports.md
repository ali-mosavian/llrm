> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ports.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ports.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ports.rs sha256:49478b5e669a2de515d44d4ec820eb108b0aeef1b67d03fe3eff2f7eb4d2e85d -->

# ports

Each I/O port call narrowed to the memory its device reaches, as the
target describes the port.

## How it works
`llrm.ia16.in` and `out` intrinsic calls with no `memory` attribute are
candidates. A call whose port range the target says reaches no memory
(`Machine::port_touches_memory`; every target answers from its platform's
port table, `pc-ports.toml` for the PC, so the 16-bit and the flat DOS
targets alike) gets `memory(inaccessiblemem: readwrite)`: it touches
only its own state, so alias and dead-store passes may move memory
operations across it, and port calls still keep their order among
themselves. A call to a port that may reach memory (a DMA controller, say)
keeps the unknown effect, so a store before it stays.

The port is a constant (masked to 16 bits), or else an interval the
manager's `Bounded` ranges give at the call's block (a branch guarding the
VGA's 960..975 bounds it); an interval that wraps through 8000h gives
nothing.

```text
 out 968, v    ; the DAC's write index
   => out 968, v  memory(inaccessiblemem: readwrite)
 out 11, v     ; the 8237's mode: may start DMA
   => unchanged
```

DEVIATE LP-1 (no pin pass: LLVM states what an I/O port reaches on the
intrinsic definition, ours asks the target per call; exit: the frontends
declare the memory attribute on the `in`/`out` intrinsics and this pass is
deleted; owner w-6; review 2026-11-13).

## Example
Before:
```mir
define void @f(i16 %p, i8 %v) {
b0:
  call void @llrm.ia16.out.i8(i16 968, i8 %v)
  call void @llrm.ia16.out.i8(i16 11, i8 %v)
  %low = icmp sge i16 %p, 960
  br i1 %low, label %b1, label %b3

b1:
  %high = icmp sle i16 %p, 975
  br i1 %high, label %b2, label %b3

b2:
  call void @llrm.ia16.out.i8(i16 %p, i8 %v)
  br label %b3

b3:
  call void @llrm.ia16.out.i8(i16 %p, i8 %v)
  ret void
}
```
After:
```mir
define void @f(i16 %p, i8 %v) {
b0:
  call void @llrm.ia16.out.i8(i16 968, i8 %v) memory(inaccessiblemem: readwrite)
  call void @llrm.ia16.out.i8(i16 11, i8 %v)
  %low = icmp sge i16 %p, 960
  br i1 %low, label %b1, label %b3

b1:
  %high = icmp sle i16 %p, 975
  br i1 %high, label %b2, label %b3

b2:
  call void @llrm.ia16.out.i8(i16 %p, i8 %v) memory(inaccessiblemem: readwrite)
  br label %b3

b3:
  call void @llrm.ia16.out.i8(i16 %p, i8 %v)
  ret void
}
```
Port 968 (the VGA's DAC write index) and the call in `b2`, which the
branches bound to the VGA's 960..975, are narrowed. Port 11 (the 8237's mode
register, which can start DMA) and the call in `b3`, whose port nothing
bounds, keep the unknown effect.

From unit test
`test_a_port_call_is_narrowed_only_where_the_target_says_its_device_reaches_no_memory`
in
`ports_tests.rs`, on the x86-16 DOS target.

## Runs
Twice: once over every body before the per-body pipeline, so a caller's
pipeline sees each callee's port calls narrowed, and as the second pass of
each round of [`crate::pipeline`](pipeline.md)'s fixed point, before anything asks what a
port call does to memory. On at every level above -O0.

## Needs
The target's port description, `Registers`, `Bounded`.

## Keeps
`Dominators` and `Loops`: only call attributes change.
