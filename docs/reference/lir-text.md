# LIR text

For anyone reading a machine-phase dump: what each line says. `ISEL_DUMP=1` prints the LIR after every machine phase of every function; `ISEL_DUMP=verbose` adds the notes listed [below](#verbose). Colour only where stdout is a terminal. [Debugging](../machine/register-allocation/debugging.md) says how to use a dump to find a phase.

One printer writes it for every consumer: the dumps, the messages of the checks, the tests and these docs. Operands and instructions are [`llrm-lir/src/text.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-lir/src/text.rs), blocks and bodies [`lirtext`](../optimizations/backend/lirtext.md). A golden test per `Operation` fixes the text of each.

```lir
═══ _f@3 · RegAlloc · m16 ══════════════════════════════════════════════════════
fn _f@3(y4.1@ax) -> @eax

B0  entry  → B1                                                   ×1.0
    #24  y2.1       = elided      y4.1
     #2  bx         = mov.i16     1
     #3  cx         = mov.i16     0
     #4               jmp         B1

    B1  ← B0 B3  → B3 B2  loop                                        ×32.0
         #8  ax         = sub.i16     ax, 1
         #9               cmp.i16     ax, -1
        #10               jne         B3 else B2                            ; 97% taken
```

From `ISEL_DUMP=1 llrm-c -m16 -O2` of a Fibonacci loop. Its locals carry the front end's ids (`y4`) until llrm-c maps the `-d2` names onto places ([#1908](https://github.com/ali-mosavian/llrm/issues/1908)); Nib and QB locals read as written.

## Lines

| Line | Says |
|---|---|
| `═══ fn · Stage · m16 ═══` | The function and the phase it was printed after; `m16` or `m32`. |
| `fn name(args) -> @regs` | The values and registers its arguments arrive in; the registers it returns in. `; calls clobber ...` is the set most of its calls clobber. |
| `frame slot0 [bp-2] 2 B ...` | Each frame cell: its name, where it is from the frame register, its size. Arguments are `arg0`. |
| `B3  ← B1 B2  → B4  loop  ×8.0` | A block: its predecessors, its successors (`→ exit` if it returns), `entry`, `loop` (and `N trips` for an innermost loop whose count is known), `cold`, and how often it runs per call. |
| `#12  dest = op.i16  operands  ; notes` | An instruction. |

A block is indented four spaces for each loop around it, so a loop is a shape on the page. Blocks are named by position in the body and every target names its block: no address appears, and two dumps of one body differ only where the instructions do. Columns are fixed, not fitted to the longest line.

## Instructions

`#12` is the instruction's label. It is given once, in emit order, and kept by every pass that keeps the instruction (a clone keeps it, a duplicate gets a new one, a number is not reused), so the dump after a pass and the one before it differ in the lines the pass changed. Phis have one too.

`dest = op.type operands`. The type is MIR's spelling of the operation's width: `i8 i16 i32 i64`, and `f32 f64 f80` for the float operations. A source of another width says so (`t10:i8`).

- **Values** are named by their source name where they have one (`count.2`, `lsr.iv`), else `t12`. A copy of a named value keeps the name while it is defined once; defined again (after the phis are gone) it is another version, and is `t12`. A web of merged values prints under one name. `t12@ax` is a value placed in a register.
- **Registers** are named by width: `al`, `ax`, `eax`, `st0`.
- **Memory** is `[es:t3@bx+t4*4+8]`: segment, base, index times scale, displacement. A frame cell is `[slot0]`, `[slot0+2]`; a global `[seg1+0x10]`. A store has its memory operand first and no `=`: `store.i16 [slot0], t7`. A load is `load`.
- **Immediates** are decimal; a mask (`and`, `or`, `xor`, `test`) is hex at its width. An address is `&slot1`, `&seg1+0x10`.
- **Control flow** names every target: `jmp B5`, `jl B5 else B4`, `switch t3 [0..2->B5, 3->B6]`, `ret t9@eax`.
- **Calls** name the callee and the values passed in registers: `t13@eax = call fib(t12@ax)`, `call far N$BDRP(t1@ax)`, and for inline code its bytes, `asm{8a 86 ...}`. An indirect call names its operand.
- `elided` is an operation selection removed that still defines or reads a value: `t5 = elided t2`.

## Notes

Only where informative: `spill` and `reload` (frame traffic), `copy` (a phi's parallel copy), `remat`, `frame` (a frame adjustment), `volatile`, `N% taken` on a branch whose odds are known, and `also clobbers esi` or `keeps es` on a call that differs from the function's usual set. Never "inserted".

## Verbose

`ISEL_DUMP=verbose` adds each block's `live-in` and `live-out`, what an instruction needs and gives in registers (`needs`, `gives`), the values it defines or reads that no operand names (`also defines`, `also uses`), the source bytes it covers and its source line (under `-g`).

## Source

Operand and instruction text: [`llrm-lir/src/text.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-lir/src/text.rs); body, block and stage layout: [`lirtext`](../optimizations/backend/lirtext.md). Labels and names live on `Insn`, `Phi` and `LirBody` ([`lir.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/model/lir.rs)); they take no part in equality, so a pass never sees them.
