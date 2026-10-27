# BASIC calling conventions

For contributors to `llrm-qb`, `llrm-omf` and the QB runtime. The convention of BASIC procedures on the real-mode target, how calls into the Microsoft and llrm runtimes are described, and the frame `B$ENRA` builds. The facts measured from compiled BASIC, with the evidence, are in [frontends/qb/abi.md](../frontends/qb/abi.md); descriptors and frame sizes by dialect are in [memory-model](../frontends/qb/memory-model.md#procedure-calls-and-frames).

## Procedures

A SUB or FUNCTION is `pascal16`'s shape ([x86-m16](x86-m16.md#the-conventions)): the first formal is pushed first, the call is far and the callee removes the arguments with `retf n`. `DECLARE ... CDECL` alone reverses the push order and makes the caller remove them. MIR sees the callee-pops case as `BASIC` and the caller-pops case as `ccc` ([selection](selection.md#from-a-convention-to-mir)); the call-site side table records the stack permutation, distance and cleanup owner, and they are applied after semantic optimization by emitting MIR `ARG` forms.

| Parameter | Passed as |
|---|---|
| `BYREF` (the default) | a near pointer into DGROUP, 2 bytes |
| `BYVAL` | the value: 2 bytes for an INTEGER, 4 for a LONG |
| array | one word, the address of the array descriptor |
| FUNCTION result | INTEGER or string descriptor in AX, LONG in DX:AX; SINGLE and DOUBLE through a near pointer the caller pushes last, returned in AX |

A procedure keeps SI, DI, BP, SP, DS, SS and the direction flag (the `keeps` rows of `tests/fixtures/callconv/bas/reference.txt`, read from BC's listings). Every other register is the callee's: a call no runtime contract describes clobbers every register except those the program declares preserved, and its memory effects stay conservative (`ordinary` in `abi/qb.rs`).

A far procedure, SUB `pDbl(h AS INTEGER, v AS DOUBLE, t AS INTEGER)`, at entry (BC's listing: `far pops 6`, the arguments at `in+8`, `in+6`, `in+4` from SP):

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','textColor':'#64748b'}}}%%
block-beta
  columns 8
  space:1 a3["▲ higher addresses"]:3 space:4
  o1[" "] c1["caller's frame"]:3 s1[" "] n1[" "]:3
  o2["sp+8"] c2["&h"]:3 s2["2"] n2["first formal, pushed first"]:3
  o3["sp+6"] c3["&v"]:3 s3["2"] n3[" "]:3
  o4["sp+4"] c4["&t"]:3 s4["2"] n4["last formal, pushed last"]:3
  o5["sp+2"] c5["return CS"]:3 s5["2"] n5[" "]:3
  o6["sp+0"] c6["return IP"]:3 s6["2"] n6["SP on entry"]:3
  space:1 a10["▼ lower addresses"]:3 space:4
  classDef off fill:transparent,stroke:transparent,stroke-width:2px,color:#64748b
  classDef cell fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff
  classDef size fill:#1e293b,stroke:#0f172a,stroke-width:2px,color:#e2e8f0
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff
  class a3,o1,n1,s1,o2,n2,o3,n3,o4,n4,o5,n5,o6,n6,a10 off
  class c1,c2,c3,c4 cell
  class s2,s3,s4,s5,s6 size
  class c5,c6 src
```

*Far BASIC procedure at entry (SUB pDbl(h, v, t))*

A private procedure that frames itself is called near: no CS, so the arguments start 2 bytes closer (`[bp+4]` after `push bp; mov bp,sp`).

## The runtime frame

A procedure whose errors can land in it, or that has a local string, calls `B$ENRA` and leaves with `B$EXSA`; any other frames itself with `push bp`, `mov bp,sp`, `sub sp`. `B$ENRA` takes CX = bytes of locals (even) and builds the frame below. Its header is 10 bytes in QB 4.5 and in the llrm runtime, 18 in PDS 7.1 and 20 in VBDOS (`_RUNTIME_FRAME_HEADER` in `crates/codegen/llrm-codegen/src/driver/basic.rs`); the llrm runtime's flat header is 32. `B$EXSA` takes BP and the result in DX:AX, restores SI, DI and BP and leaves BP and SP as the procedure used them.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','textColor':'#64748b'}}}%%
block-beta
  columns 9
  space:1 a3["▲ higher addresses"]:3 space:5
  o1[" "] c1["arguments"]:3 s1[" "] b1_0[" "] n1[" "]:3
  o2["bp+4"] c2["return CS"]:3 s2["2"] b2_0[" "] n2[" "]:3
  o3["bp+2"] c3["return IP"]:3 s3["2"] b3_0[" "] n3[" "]:3
  o4["bp+0"] c4["caller's BP"]:3 s4["2"] b4_0[" "] n4["BP points here"]:3
  o5["bp-2"] c5["previous BASIC frame"]:3 s5["2"] b5_0["┐"] n5["b$curframe chain"]:3
  o6["bp-4"] c6["SI"]:3 s6["2"] b6_0["│"] n6[" "]:3
  o7["bp-6"] c7["DI"]:3 s7["2"] b7_0["│"] n7["<b>runtime header</b>"]:3
  o8["bp-8"] c8["bytes of locals"]:3 s8["2"] b8_0["│"] n8[" "]:3
  o9["bp-10"] c9["GOSUB count"]:3 s9["2"] b9_0["┘"] n9["header ends: FR_SIZE = 10"]:3
  o10["bp-12"] c10["locals, zeroed"]:3 s10[" "] b10_0[" "] n10["SP at the bottom"]:3
  o11[" "] c11["..."]:3 s11[" "] b11_0[" "] n11[" "]:3
  space:1 a15["▼ lower addresses"]:3 space:5
  classDef off fill:transparent,stroke:transparent,stroke-width:2px,color:#64748b
  classDef cell fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff
  classDef size fill:#1e293b,stroke:#0f172a,stroke-width:2px,color:#e2e8f0
  classDef gap fill:#334155,stroke:#64748b,stroke-width:2px,color:#e2e8f0,stroke-dasharray:4 3
  class a3,b1_0,o1,n1,s1,b2_0,o2,n2,b3_0,o3,n3,b4_0,o4,n4,b5_0,o5,n5,b6_0,o6,n6,b7_0,o7,n7,b8_0,o8,n8,b9_0,o9,n9,b10_0,o10,n10,s10,b11_0,o11,n11,s11,a15 off
  class c1,c4,c5,c6,c7,c8,c9,c10 cell
  class c2,c3 src
  class s2,s3,s4,s5,s6,s7,s8,s9 size
  class c11 gap
```

*Frame B$ENRA builds (llrm runtime, 16-bit)*

The diagram is the llrm runtime's, `runtime/qb/dos/m16/frame.asm`; its flat counterpart is `runtime/qb/dos/m32/frame.asm`. The backend's names for the entries are `ENTER` and `LEAVE` in `backend/frame.rs`.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','actorBkg':'#2563eb','actorBorder':'#1e40af','actorTextColor':'#ffffff','actorLineColor':'#94a3b8','signalColor':'#64748b','signalTextColor':'#64748b','noteBkgColor':'#ea580c','noteBorderColor':'#9a3412','noteTextColor':'#ffffff','activationBkgColor':'#cbd5e1','activationBorderColor':'#64748b','lineColor':'#64748b'},'sequence':{'mirrorActors':false,'messageAlign':'center','actorMargin':70,'boxMargin':10,'width':170,'height':46,'noteMargin':12}}}%%
sequenceDiagram
  participant c as caller
  participant p as PROC
  participant r as runtime
  Note over c: push &arg0 ... &argN-1
  c->>p: call far PROC
  p->>r: call far B$ENRA<br/>(CX = locals)
  r-->>p: build frame,<br/>zero locals
  Note over p: body
  p->>r: call far B$EXSA
  r-->>p: restore SI DI BP
  Note over c: ✓ nothing to pop
  p-->>c: retf n
```

*A BASIC procedure call through the runtime*

## Calls into a runtime

| `--runtime` | A runtime call is described by |
|---|---|
| `qb45`, `pds71`, `vbdos` | the Microsoft routine's audited stack contract: bytes it removes, registers it destroys and reads, memory and error effects (`llrm-qbruntime` tables, with the `_AUDITED_*_STACK` tables in `abi/qb.rs` for the cleanup of the string, graphics and statement entries). A routine without a complete contract is refused. |
| `llrm` (default) | the target's own convention: the routines are plain public C functions in `runtime/qb`, far in the medium model, with link names given by `#pragma aux` (`runtime/qb/qb.h`). Only the table's effects are kept, not Microsoft's stack block or registers. |

Some entries keep a register interface beside the stack block: `B$HARY` takes the subscripts and their count pushed and the descriptor in BX and answers the element's address in ES:BX (`registers` in `abi/qb.rs`). `B$RESA` takes the resume offset in AX.

## Calls out of BC objects

`llrm-omf` lifts a Microsoft BASIC object. For a call to an external, non-`B$` symbol with no runtime contract, `callsite.rs` assumes caller cleanup when the next instruction is `add sp, imm` and callee cleanup otherwise, and keeps every general-purpose input.

## Tests

`crates/frontends/qb/src/callconv_tests.rs` compares llrm's lowering of each call boundary with BC's `/A` listings of the same program; [conformance](conformance.md) lists the rest.
