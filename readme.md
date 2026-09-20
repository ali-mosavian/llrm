# qbopt

`qbopt` rewrites the OMF `.OBJ` that QuickBASIC, PDS, or VBDOS produces before
it is linked. It raises BC's 8086 code into SSA-based MIR, optimizes whole
functions, then lowers and allocates it for a 386-or-later target.

The goal is output within 1.5× a hand-derived modern-compiler listing for
every suite program. See [targets](docs/targets.md) for the evidence and
current gaps.

## Use

Python 3.13+ and [uv](https://docs.astral.sh/uv/) are required.

```sh
uv sync
uv run python -m qbopt.rewrite PROGRAM.OBJ -o PROGRAMQ.OBJ --cpu 386
LINK PROGRAMQ.OBJ
```

Pass every object and library in the same order as the LINK invocation when
the program spans modules. qbopt resolves each external against that complete
link unit, optimizes every standalone object, and writes only after all of them
have completed:

```sh
uv run python -m qbopt.rewrite MAIN.OBJ DRAW.OBJ BCOM45.LIB \
  --output-dir build/optimized --cpu 386
LINK build/optimized/MAIN.OBJ+build/optimized/DRAW.OBJ,,,BCOM45.LIB
```

For a resolved definition whose audited runtime interface is not already
known, qbopt follows the OMF call graph and conservatively discovers which
entry-register values can reach a read. Recursive, indirect, ambiguous, and
otherwise unresolved edges consume all allocatable GP inputs. This narrows
only the call's input liveness; memory, clobber, control, cleanup, and error
effects remain opaque.

Unsupported OMF records, unresolved or ambiguous externals, incomplete
lowering, allocation failures, and unencodable instructions are errors by
default. `--allow-unchanged` is the explicit compatibility mode for retaining
an input object when its backend refuses it. With multiple objects, qbopt
finishes the entire link unit in memory before creating any output, so a
failure cannot leave a partly optimized set behind.

Useful options:

```sh
# Keep BASIC numeric runtime behavior, including its conversion/error paths.
uv run python -m qbopt.rewrite PROGRAM.OBJ -o PROGRAMQ.OBJ --basic-semantics

# Preserve array checking; omit checks proven unnecessary. Independent of numeric semantics.
uv run python -m qbopt.rewrite PROGRAM.OBJ -o PROGRAMQ.OBJ --bounds-checks

# Use real x87 instead of BC's emulator interrupt protocol.
# Requires a coprocessor; incompatible with --basic-semantics.
uv run python -m qbopt.rewrite PROGRAM.OBJ -o PROGRAMQ.OBJ --native-fpu

# Inspect a rewrite or dump every pipeline stage.
uv run python -m qbopt.rewrite PROGRAM.OBJ --report
uv run python tools/stages.py PROGRAM.OBJ --dump build/stages/PROGRAM
```

Audited project calls can be supplied with `--contracts PROFILE.json`
and `--contract-root OBJECT_DIRECTORY` (also supported by `tools/stages.py`).
The loader checks every artifact's SHA-256 and each symbol's defining object
before enabling register-input/stack-cleanup facts. Unspecified effects stay
unknown; this does not automatically prove contracts. See
[the profile format](docs/contracts/README.md).

Native arithmetic is the default, but it is not fast-math: floating-point
reassociation and observable storage rounding are not discarded. Native LONG
division uses machine/C behavior; `--basic-semantics` retains `B$DVI4` instead.

## Optimizations

```text
OMF -> decode -> raise -> MIR passes -> lower -> register allocation -> peephole -> OMF
```

The raise recognizes BC-specific LONG pairs, runtime arithmetic calls, and
supported numeric array descriptors. MIR passes are machine-independent;
only lowering, allocation, and peephole work with registers or instructions.
See [the MIR boundary](docs/split.md).
The [compiler foundations plan](docs/compiler-foundations.md) defines the
correctness and code-quality goals, ownership boundaries, and delivery order.

Current work includes:

- LONG widening; constant folding/propagation; branch and dead-code removal;
- CSE, load/store forwarding, promotion, and proven runtime-call memory facts;
- LICM, induction variables, affine address recurrences, and strength reduction;
- native static, dynamic FAR, and HUGE numeric array addressing, 1–60 dimensions;
- spill folding, constant rematerialization, reload removal, and post-allocation cleanup.

Unsupported array layouts refuse unchecked lowering. Checked loop preguards
and broader strict-FP optimization are still unfinished.

## HARR: before and after

`suite/harr.bas` stores and immediately rereads a two-dimensional INTEGER
array element. The helper is `B$HARY`; `harr` is the benchmark name.

Before, BC performs the address calculation twice per inner iteration:

```asm
push column
push row
push 2
push descriptor
call B$HARY                 ; returns ES:BX
mov  [es:bx],value

push column
push row
push 2
push descriptor
call B$HARY                 ; recomputes the same address
mov  ax,[es:bx]
add  [total],ax
```

Its effective offset is:

```text
((column - lowerColumn) * rowCount + row - lowerRow) * elementSize + base
```

After CSE, LICM, forwarding and induction lowering, descriptor setup is
outside both loops:

```asm
mov  bx,[descriptor]
mov  es,[bx+2]              ; selector loaded once
mov  dx,44
add  dx,[bx+10]
mov  bx,dx                  ; outer pointer
```

The hot inner loop has no helper, multiply, descriptor load, selector reload,
or array reread:

```asm
inner:
mov  [es:di],si             ; matrix(row,column) = row + column
add  cx,si                  ; reuse the just-stored value
add  si,1                   ; column/value induction
add  di,42                  ; 21 INTEGERs * 2 bytes
cmp  si,dx
jne  inner
```

The outer latch uses `add bx,2`. `di` and `bx` are the inner and outer address
recurrences; `si` is the `row + column` recurrence.

## Validate

```sh
uv run pytest
uv run pytest --full -n 4 -m "not e2e"
uv run pytest --full tests/test_array_access.py
uv run python tools/e2e.py p-g2 --prog harr
uv run python -m qbopt.price PROGRAM.OBJ
```

For every failure, dump every stage and diff the first changed pair. Every fix
needs a regression that fails before the fix. Testing details are in
[docs/testing.md](docs/testing.md); current measured progress is in
[docs/takeover-progress.md](docs/takeover-progress.md).
