> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/affine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/affine.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/affine.rs sha256:07d344283edf5b4107d4ad88bcc308a19538f0c481b603f95ae1c7a6ab3ad7fa -->

# affine

Recognises allocated instructions that keep a register an affine sum of
registers, and names the 32-bit or 16-bit address of such a sum.

## How it works
A register is affine when each write to it is a copy, an add of a
constant, a scale or an add of another register: `step` classifies one
instruction as `Copy`, `Add`, `Scale` or `AddRegister` and prices it from
the CPU profile. Only 16- and 32-bit integer registers qualify, and an
instruction with side effects beyond its operands (`plain`) never does.

A chain of such steps leaves a register equal to `sum(multiple * reg) +
disp`, kept as `Terms`. `form` turns terms into the address-size-
prefixed 32-bit address that names them (`[base+index*scale+disp]`, the
index never the stack pointer, the scale one the target's form takes).
`word_form` does the same for real-mode addressing: a lone BX/BP/SI/DI,
or one base and one index, with no scale and no prefix.

```text
 mov si,ax ; shl si,1 ; add si,4     terms [(ax,2)], disp 4
                                     form  [eax*2+4]  (67h)
```

## Used by
`exactaddress.rs`, which folds the chain into a memory operand, and the
peephole's address chains (`peephole.rs`), which replace a chain by one
`lea`. Each does so only where the address is cheaper than the chain.
