> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ports.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ports.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ports.rs sha256:d67f0ed023e05f24271ea37ea5d3772bfef57d68915591bebbd368c4a2f0a2a8 -->

# ports

Each I/O port call narrowed to the memory its device reaches, as the
target describes the port.

## How it works
`llrm.ia16.in` and `out` intrinsic calls with no `memory` attribute are
candidates. A call whose port range the target says reaches no memory
(`Machine::port_touches_memory`; the x86-16 target reads each device's reach
from its description) gets `memory(inaccessiblemem: readwrite)`: it touches
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

## Runs
Twice: once over every body before the per-body pipeline, so a caller's
pipeline sees each callee's port calls narrowed, and as the second pass of
each round of [`crate::pipeline`](pipeline.md)'s fixed point, before anything asks what a
port call does to memory. On at every level above -O0.

## Needs
The target's port description, `Registers`, `Bounded`.

## Keeps
`Dominators` and `Loops`: only call attributes change.
