# BC objects as input

Audience: users and contributors of `llrm-omf`. How a Microsoft BASIC compiler
object (QB 4.5, PDS 7.1 or VBDOS `BC` output) is recompiled with llrm's
optimizer and backend. Source programs go through [llrm-qb](qb/readme.md)
instead; this route is for programs whose source is not at hand.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  obj["<b>BC .OBJ ...</b><br/><small>[runtime .LIB]</small>"]:::src
  bc["<b>llrm-bcobj</b><br/><small>decode: declen, blocks, extent<br/>find the module header<br/>raise each body to MIR</small>"]:::opt
  mir["<b>MIR: one function per body</b><br/><small>main, each procedure by name<br/>DGROUP variables are globals<br/>runtime routines are<br/>@llrm.qb.* declarations</small>"]:::mid
  sh["<b>shared optimizer, isel,</b><br/><small>machine phases<br/>(as for any frontend)</small>"]:::mid
  out["<b>fresh BASIC-envelope OMF</b><br/><small>object, one per input</small>"]:::out
  obj --> bc
  bc --> mir
  mir --> sh
  sh --> out
```

*A BC object recompiled*

## Usage

```
llrm-omf OBJ... [LIB...] -o OUT [--manifest M] [-march=CPU]
```

The objects are one program in LINK order. `-o` names the output object, or a
directory when there are several inputs. Libraries are the runtime and are
not recompiled. `--manifest M` writes a small JSON note of the route.
`-march` and the other target options are the shared ones. The driver is
`crates/driver/llrm-bcdriver`.

## What it does

It is a recompile, not a rewrite in place. Every function is raised to MIR,
optimized, selected and emitted again, and a fresh object is written around
the code. A function the raiser cannot express fails the whole program and
nothing is written. The runtime family is read from the object (QB 4.5, PDS
7.1 or VBDOS); other objects are refused.

The module header (`MODULE_CODE`, 0x30 bytes at the start of the code
segment, whose signature word names the family) is kept as BC wrote it except
for the statement-table and main-frame words, which are written afresh. A
header word relocated other than as BASIC relocates it is refused.

## What the raiser decodes

- The FP emulator's interrupts `34h` to `3Bh` stand in for x87 ESC opcodes
  with the operand inline; `3Ch` stands in for a segment-override prefix and
  `3Dh` for `WAIT`. `crates/frontends/bcobj/src/frontends/bc/declen.rs`
  and `fppatches.rs` decode each for what it is.
- `ON k GOTO` is `call far B$OGTA` followed by inline data: a count byte and
  that many offsets. The runtime reads the return address to find the table, so
  that call does not return to the byte after it
  (`INLINE_TABLE` in `llrm-qbruntime`).
- BC compiles a statement at a time, so no value outlives a statement in its
  code; the raiser sees each body as MIR, not as BC's instruction sequence.
- BC writes an object even after severe errors, so an object's existence is
  not evidence that its source compiled.

## Tests

`crates/driver/llrm-bcdriver/tests/driver.rs` recompiles BC fixtures and checks the
header, constants, statement table, refusals and instruction sequences.
`tests/differential/` compares llrm-qb with BC on source programs
([testing](../working/testing.md)).
