# MIR is x86 with SSA names on it

Counted over `fixtures/omf/*-p-g2.obj`, 2,343 operations:

```
  PUSH 740, POP 15      the machine stack as an operation. A call takes
                        arguments; there is no push in an IR.
  flags 762 defines,    `cmp` writes a flags value and `jle` reads it.
        566 uses        Three-address form is `c := a <= b` then `branch c`.
  BINARY 155,           not operations. Which one it is lives in `name`, an
  UNARY 106             x86 mnemonic -- so add, and, shl and adc are one op.
  adc, sbb, cwd,        idioms of splitting a wide value in half, and `wait`
  wait, restore         is an x87 sync instruction with no meaning at all.
  FLOAT_* 79            the x87 stack machine. St(index) has no MIR form and
                        is carried as mir.Opaque.
  ir.Operation          the decoder's own enum, shared with select, lir and
                        regalloc. BINARY's docstring says "sources[0] IS
                        dests[0]" -- two-address x86, by construction.
```

## What it should be

Three-address over values with widths, and nothing else:

```
  c := a + b          ADD SUB MUL DIV REM AND OR XOR SHL SHR SAR NEG NOT
  c := a <= b         LT LE GT GE EQ NE -- a value, never a flag
  c := load m         LOAD
  store m, a          STORE
  c := a              COPY
  c := conv a         CONVERT -- a width and a signedness, not cwd
  c := call f(a, b)   CALL -- arguments, not a push run
  branch c, x, y      BRANCH
  goto x              JUMP
  return a            RETURN
```

No register pairs, no flags, no stack, no mnemonics.

## The work is the raise, not the passes

Every one of BC's idioms has to be recognised once, in `raise_body`, and
become one operation. That is what rule 5 means by "what an idiom *is* is
the raise's answer": a long add arrives as `add` then `adc` and leaves as
one ADD at width 4; a call arrives as a push run and leaves as CALL with
arguments; `cmp` then `jle` leaves as LE then BRANCH.

## Order

1. **`mir.Kind`** -- MIR's own operation set, classified at the raise.
   `add`, `and` and `shl` stop being one operation called BINARY. **Done.**
   With it: `Op.test` -- which comparison a branch asks -- `Op.stack` for
   the float unit's depth, `Opaque.name` for a resource MIR has no value
   for, and `inc`/`dec` writing down the one they add. segments.py and
   fpstack.py name nothing about the machine at all; consts.py, avail.py
   and wide.py name it only in prose.
2. **Comparisons are values.** `Op.test` is half of this; the other half
   is the compare and the branch becoming one operation, which needs
   lowering to emit two instructions for one. `c := a <= b`, `branch c`. Removes 1,328
   flags mentions and the FLAGS pseudo-register with them.
3. **Calls take arguments.** The push run before a call is recognised at
   the raise. Removes 740 PUSH.
4. **Wide arithmetic is one operation.** `add`/`adc` becomes ADD at width
   4 -- `widen` moved into the raise, which the plan already wanted.
5. **x87 over values.** The stack rotation resolved at the raise, which
   is what removes `mir.Opaque`.
6. **Delete** `node`, `made`, `covers`, `origin` and `ir.Operation` from
   MIR. The deletion is what makes the claim true; the conversion only
   makes the deletion possible.

## What is left, and why each is blocked

```
  transform.py  the absorb builders (_absorbing, _comparing, _deleting,
                made) -- 65 references, and phase D moves them into the
                raise, which is where they belong
                the hoist's allocator (hoisted, _insertion, _move,
                _instead, _writes_to) -- S1, and it needs regalloc to
                split a live range on LIR first: removing it blind
                miscompiled hotlop on all twelve configurations
                the register-level liveness the hoist needs (_carried,
                _leaving, _placed, widths) -- a 16-bit write under a
                32-bit register model is a read-modify-write, and that
                artifact goes when a wide value is one value
  pairs.py      BC's ax:dx pair identity -- step 4, wide arithmetic at
                the raise
  mir.Op        node, made, covers. node and raised could move to a side
                table today; made and covers cannot, because the three
                blocks above still write them.
  MirBody       origin -- last, as the plan says
```

Each step: 485 of 485 rebuild and the corpus byte total through
`rewrite.py`, which is the shipped optimiser. `wholeseg.rebuilt` is not,
and measuring through it is what hid a miscompile for five commits.
