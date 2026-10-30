> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/affine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/affine.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/affine.rs sha256:8b77a5ec48b13b93ea310fd297607f584cabd1612198cc788eec9703938be12c -->

# affine

Recognises allocated instructions that keep a register an affine sum of
registers, and names the 32-bit or 16-bit address of such a sum.

## How it works
A register is affine when each write to it is a copy, an add of a
constant, a scale or an add of another register: `step` classifies one
instruction as `Copy`, `Add`, `Scale` or `AddRegister` and prices it from
the CPU profile. Only 16- and 32-bit integer registers qualify, and an
instruction with side effects beyond its operands (`plain`) never does.

A chain begins at a copy, or at a symbol-free `lea` (`lea_start`), and its
steps may be words or the dwords the target ran them as (`promote`). A
chain of such steps leaves a register equal to `sum(multiple * reg) +
disp`, kept as `Terms`. `form` turns terms into the address-size-
prefixed 32-bit address that names them (`[base+index*scale+disp]`, the
index never the stack pointer, the scale one the target's form takes).
`word_form` does the same for real-mode addressing: a lone BX/BP/SI/DI,
or one base and one index, with no scale and no prefix.

```text
 mov si,ax ; shl si,1 ; add si,4     terms [(ax,2)], disp 4
                                     form  [eax*2+4]  (67h)
```

## Example
A copy and a shift that make `cx*2`, before and after the peephole's address
chain. `; ...` marks lines left out; the `; inserted` notes are dropped.

Before (`ParallelCopy`):

```lir
0x0000  CX := mov [None v2@SI]
; ...
0x0007  BX := mov CX
0x0007  BX := shl BX, 0x1
```

After (`Peephole`):

```lir
0x0000  CX := mov [None v2@SI]
; ...
0x0007  BX := lea Address(addr=None, through=38, index=38, scale=1, offset=0, disp_width=0)
```

The chain is one `lea` in the listing: `lea bx, [ecx+ecx]`.

From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of a BASIC function that sums an
INTEGER array (`Total%`).

## Used by
`exactaddress.rs`, which folds the chain into a memory operand, and the
peephole's address chains (`peephole.rs`), which replace a chain by one
`lea`. Each does so only where the address is cheaper than the chain.
