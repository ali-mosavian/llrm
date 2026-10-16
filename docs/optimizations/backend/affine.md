> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/affine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/affine.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/affine.rs sha256:63728630b46ebc099dfa7a89e6f686a6e307f54e63b8f8ba39882a31f1cfe54f -->

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
A copy and a shift that make `dx*2`, before and after the peephole's address
chain. `; ...` marks lines left out.

Before (`ParallelCopy`):

```lir
        #12  dx         = load.i16    [t8:S%.1.lcssa@bx+t24@di+0x2]
; ...
        #36  bx         = mov.i16     dx
        #13  bx         = shl.i16     bx, 1:i8
```

After (`Peephole`):

```lir
        #12  dx         = load.i16    [t8:S%.1.lcssa@bx+t24@di+0x2]
; ...
        #36  bx         = lea.i16     &+edx+edx*1
```

The chain is one `lea` in the listing: `lea bx, [edx+edx]`.

From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of a BASIC function `Total%` that
sums `a(i) + a(a(i))` over an INTEGER array.

## Used by
`exactaddress.rs`, which folds the chain into a memory operand, and the
peephole's address chains (`peephole.rs`), which replace a chain by one
`lea`. Each does so only where the address is cheaper than the chain.
