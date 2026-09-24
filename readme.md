# llrm

`llrm` is a compiler suite for 16-bit real-mode DOS that aims at GCC/LLVM-quality
code. Every frontend raises to one SSA IR (MIR), shares one optimizer and x86
backend, and writes linkable OMF `.OBJ` files.

```text
source or .OBJ -> frontend -> MIR passes -> lower -> register allocation -> peephole -> OMF
```

MIR passes are machine-independent; only lowering, allocation and peephole see
registers or instructions. See [the MIR boundary](docs/split.md). The goal is
output within 1.5× a hand-derived modern-compiler listing for every suite
program; [targets](docs/targets.md) has the evidence and current gaps.

## Frontends

| Tool | Input |
| --- | --- |
| `llrm-qb` | QuickBASIC-family source: QB 4.5, QBasic 1.1, PDS 7.1, VBDOS |
| `llrm-c` | C, through a patched Open Watcom front end (`owshim/`) |
| `llrm-modern` | llrm's own language; see [the language](docs/modern-language.md) |
| `llrm-omf` | OMF objects produced by QuickBASIC's BC, rewritten in place |

`llrm-c` and `llrm-omf` tune with `--cpu`, 386 through Core. Floating point is native x87, so a
coprocessor is required.

## Use

```sh
cargo build --release
target/release/llrm-qb PROGRAM.BAS --dialect qb45 --runtime qb45 -o PROGRAM.OBJ
target/release/llrm-c program.c --opt -o PROGRAM.OBJ
target/release/llrm-modern program.mod -o PROGRAM.OBJ
target/release/llrm-omf PROGRAM.OBJ -o PROGRAMQ.OBJ --cpu 486
```

`llrm-omf` takes every object and library in LINK order when a program spans
modules, and writes nothing unless all of them succeed:

```sh
target/release/llrm-omf MAIN.OBJ DRAW.OBJ BCOM45.LIB --output-dir build/optimized
LINK build/optimized/MAIN.OBJ+build/optimized/DRAW.OBJ,,,BCOM45.LIB
```

Unsupported records, unresolved externals and unencodable code are errors;
`--allow-unchanged` keeps a refused object as it was. `--basic-semantics` keeps
BASIC's numeric runtime errors and conversions, and `--bounds-checks` keeps
array checks. Audited external calls come from `--contracts PROFILE.json`; see
[the profile format](docs/contracts/readme.md).

## Optimizations

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
cargo test --release <name>
uv run python tools/port_diff.py FIXTURE
```

For every failure, dump every stage and diff the first changed pair. Every fix
needs a regression that fails before the fix. Testing details are in
[docs/testing.md](docs/testing.md).

## Python reference

`qbopt/` is the Python compiler the Rust crate was ported from. It is legacy
and kept only as the reference `tools/port_diff.py` diffs stage dumps against;
[the port map](docs/port-map.md) tracks every module.
