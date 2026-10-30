> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/frameescape.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/frameescape.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/frameescape.rs sha256:f458c44a2fcb7df3219349e9cd08e2610df3c14bb1dd4f9bb29afb706cd5364f -->

# frameescape

Which frame objects' addresses reach where, and which are exposed.

## How it works
A frame object is an `alloca`, named by its result. An address moves
through `getelementptr`, `bitcast`, `addrspacecast` and `freeze` (one
operand in, one out) and through phis. Everything else that uses an address
either accesses through it or exposes it:

- a load's pointer and a store's pointer (not the stored value) are
  accesses;
- a lifetime marker, or a use `marker` says keeps no copy (`nocapture`),
  names an object without handing out its address;
- any other use, including storing the pointer, passing it to a call,
  returning it, `ptrtoint`, or comparing it, exposes it.

`exposes(function, alloca, marker)` follows the alloca's uses through moves
and answers true at the first exposing use. `exposed_allocas` does it for
every alloca in one pass: it marks each value that has an exposing use,
then propagates back along the move edges, so an address that reaches an
exposing value through moves is exposed too. `analysed` computes the same
by fixed point, as `origins` (the allocas each value may hold the address
of) and `exposed`; only tests call it.

```text
 %a = alloca            %a --gep--> %b --phi--> %c
 %b = gep %a, 4         load %b          access: not exposure
 %c = phi [%b, ..]      call @f(%c)      exposure: %a is exposed
```

`framed(function)` is the other question: which values hold an address
inside frame objects on every path, and which objects. C's pointer
arithmetic stays inside its object, so an address the body took of a local
and moved by an integer still reaches only that local's bytes. It is
optimistic: phis and moves start unrefuted, a value is refuted when it has
a non-address definition (a parameter, a load, a call) or an input that is
a number, and refutation repeats until what remains is stable.

## Example
Storing a slot's address exposes that slot; storing through a slot does not
expose it:

```mir
define void @f() {
b:
  %kept = alloca ptr
  %leaked = alloca i16
  store ptr %leaked, ptr %kept
  ret void
}
```

`analysed(f).exposed` is `{%leaked}`.

From unit test
`storing_a_slots_address_exposes_it_but_storing_through_it_does_not` in
`frameescape.rs`.

## Provides
`exposed_allocas`, `exposes`, `framed`, `analysed`. Consumers:
[`crate::memory`](memory.md) (`object_of` marks an object captured when exposed), the
manager entry [`crate::manager::ExposedFrames`](manager.md), [`crate::interprocedural`](interprocedural.md)
(`framed`) and the backend's `isel` (which frame objects are exposed).
`scans` counts `exposes` calls, for tests.
