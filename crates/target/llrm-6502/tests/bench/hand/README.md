# Hand-written cc65 routines as C kernels

For each routine F of cc65's library (a local clone of cc65, commit a162565, its src directory), the bench
compares the hand assembly with the same job written in C and compiled by cc65 -O and by llrm-6502.
Files, all in this directory:

- `F.s`      the cc65 source, copied verbatim from the clone (`libsrc/...`), header intact.
- `F.c`      the C kernel `k_F`: plain, idiomatic C of F's documented behaviour. Not a transliteration of
             the assembly, no tricks, no calls to other library functions. Only `char`, `unsigned char`,
             `unsigned`, `int` and pointers (16-bit int: it must compile the same under cc65 and
             `clang --target=msp430`). One kernel function per file, no headers.
- `F.check.c` a cc65 program (`int main(void)`) linking `data.c`, `F.c` and `F.s`: runs the hand routine
             and `k_F` on at least 8 inputs, edge cases included (empty string, length 0, 255, 256,
             high-bit bytes, equal and unequal, the first and last position), compares results AND the
             memory they write, and returns 0 when all agree, else the number of the first case that does not.
- `F.spec`   how the bench runs it (below).

Check in a scratch copy, never in this directory. `cl65 -c F.c` writes an intermediate `F.s` that would
overwrite the hand source, and `F.c` and `F.s` give the same `F.o`. Compile the C and assemble the assembly
in separate directories, then link:

    S=$(mktemp -d); rm -rf $S; mkdir -p $S/c $S/a; H=$PWD
    (cd $S/c && cl65 -t sim6502 -O -c $H/F.check.c $H/data.c $H/F.c)
    (cd $S/a && cl65 -t sim6502 -c $H/F.s)          # and $H/F.glue.s when there is one
    cl65 -t sim6502 -o $S/F.bin $S/c/*.o $S/a/*.o && sim65 $S/F.bin; echo $?

After any check, `cmp` the `F.s` here against its file in the clone; they must be identical.

A routine whose symbol is not C-callable (runtime names such as `tosmulax`, `aslax3`, `mulax3`: no leading
underscore, arguments in A/X and on the stack) is called from `F.check.c` through a two-line glue
`F.glue.s` exporting `_hand_F`. Add it to the command line.

Calling convention: cc65 passes the last argument in A/X (a byte in A) and pushes the others on the C stack
left to right; the callee pops them. A library routine already does this, so the bench driver calls it
directly. The bench sees only 8- and 16-bit arguments and results (result in A, or A/X). A routine with a
`long` argument or result is out of scope: skip it and say so.

## Inputs

The bench driver provides these symbols; use them as pointer arguments (`data.c` has the same contents):
`buf` (16 bytes 3 1 4 1 5 9 2 6 5 3 5 8 9 7 9 3), `str` ("hello world"), `tab` (2 5 8 11 14 17 20 23),
`mix` ("Hello, World 42!"), `mix2` ("hELLO, wORLD 42?"), `num` ("  -4829xyz"), `pat` ("o w"),
`set` ("lo"), `dst` (64 writable bytes, zero). The bench calls the kernel once for the answer, then 101
times for the cycle count, with the same arguments: pick arguments for which that does not overflow `dst`
or make the answer drift (idempotent in effect; say in the spec if a call changes its own input).

## F.spec

Eight lines, `key: value`:

    name: strlen
    hand: _strlen          symbol of the hand routine as linked (runtime ones have no underscore)
    kernel: k_strlen       C function in F.c
    args: P(str,0)         B(n) byte, W(n) word, P(sym,off) address of a symbol plus offset; in call order
    wide: 1                1 when the result is 16 bits (A low, X high), 0 for a byte or nothing
    rel: -                 for a pointer result, the symbol it is measured from (result - sym); else -
    expect: 11             the answer of the first call (rel applied)
    note: ...              one line: what F does, anything odd

`expect` comes from running the HAND routine on sim65 (print it from a check variant), and must agree with
the host gcc run of `k_F` (compile `F.c` for the host with 16-bit-width types spelled out if it matters)
and with the documented behaviour. Three agreeing sources, not one.

## Rules

- Doubt the measurement: if the hand routine and your C disagree, find out which is right (read the
  assembly and cc65's documentation of the function) before changing either; report any real cc65 bug.
- A routine that cannot be put in this form (needs a long, I/O, a stack trick, a cc65-internal
  convention): skip it, and report why. Do not bend the form.
- No heavy jobs: the machine is busy. One-off cl65/ca65/sim65/gcc runs only.
- Write nothing outside this directory except a temporary directory; leave no .o or generated .s here.
