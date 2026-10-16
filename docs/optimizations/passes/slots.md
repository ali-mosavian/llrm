> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slots.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slots.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slots.rs sha256:da38b62135bc3beb87e7e298676fa04a8fc1f7a0f3b9ae8aebebed7e6d4b2d5a -->

# slots

A slot nothing reads goes with its stores: the owner of "this slot is
write-only", asked by the passes that take the last read of one away
(`gvn` forwarding a load, `promote` replacing it), so that the stores and
the alloca go in the same step and not a round later in `dse`.

## How it works
An `alloca` qualifies when every use of it, and of the addresses taken from
it by `getelementptr`, is the address of a non-volatile store. Anything else
(a load, a call, a phi, a cast, the slot's address stored as a value) may
read it, or let it be read, and the slot stays. The stores go, then the
addresses, then the slot.

```text
 %s = alloca i16                      (nothing reads %s)
 store i16 %x, ptr %s          ==>    (all three gone)
```
