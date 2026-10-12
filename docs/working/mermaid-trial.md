# Mermaid figures: trial

Audience: the maintainers choosing how docs draw figures. GitHub renders a `mermaid` fence in markdown, sized to the page, in its light and dark themes, with no generated file. This page draws four of our figures that way, with styling by classes. Compare with the generated PNG figures in #1546.

## Pipeline (graph with groups)

```mermaid
flowchart TB
  classDef src fill:#dbeafe,stroke:#2563eb,color:#1f2328
  classDef mid fill:#f6f8fa,stroke:#8c959f,color:#1f2328
  classDef opt fill:#ffedd5,stroke:#c2410c,color:#1f2328
  classDef out fill:#dcfce7,stroke:#15803d,color:#1f2328
  subgraph front["Source front ends"]
    direction LR
    qb["QuickBASIC<br/>llrm-qb"]:::src
    nib["Nib<br/>llrm-nib"]:::src
    c["C<br/>llrm-c"]:::src
  end
  qb --> hir
  nib --> hir
  c --> hir
  hir["HIR<br/>llrm-hir"]:::mid -->|mir::emit| mir["MIR<br/>llrm-mir"]:::mid
  mir --> opt["Optimizer<br/>llrm-transforms"]:::opt
  opt -->|lowering boundary| sel
  subgraph back["Backend"]
    sel["Instruction selection<br/>LIR over virtual values"]:::mid --> ph["Machine phases<br/>register allocation, peephole"]:::mid --> obj["Object writer<br/>OMF, ELF, COFF, Mach-O"]:::out
  end
```

## Frame of a QuickBASIC procedure (stack layout)

```mermaid
flowchart TB
  classDef cell fill:#f6f8fa,stroke:#8c959f,color:#1f2328
  classDef ret fill:#dbeafe,stroke:#2563eb,color:#1f2328
  classDef hdr fill:#ffedd5,stroke:#c2410c,color:#1f2328
  top(["higher addresses ↑"]):::cell
  f1["bp+8 · formal 1 · 2 bytes"]:::cell
  f2["bp+6 · formal 2 · 2 bytes<br/>last formal: nearest the return address"]:::cell
  ra["bp+2 · return address · 4 bytes<br/>far: offset, then segment"]:::ret
  hd["bp · runtime header · 10 bytes<br/>saved BP, frame chain, SI, DI (QB 4.5)"]:::hdr
  lo["locals and spills<br/>SP is at the bottom"]:::cell
  bot(["stack grows down ↓"]):::cell
  top ~~~ f1 ~~~ f2 ~~~ ra ~~~ hd ~~~ lo ~~~ bot
```

## Borrow timeline (spans over source lines)

```mermaid
gantt
  title A change while a shared borrow is used later
  dateFormat X
  axisFormat %s
  section v (owner)
  owner of v :done, v, 1, 9
  section r = &v[0]
  shared borrow, live to its last use :active, r, 3, 9
  v.push(3) is refused :crit, c, 6, 7
```

Lines: 1 `let mut v`, 3 `let r = &v[0]`, 6 `v.push(3)`, 9 `print(r)`.

## Call and return (sequence)

```mermaid
sequenceDiagram
  participant C as caller
  participant F as callee
  C->>C: push arguments (Pascal order)
  C->>F: call far (pushes CS:IP)
  F->>F: push bp, mov bp, sp, sub sp, locals
  F-->>C: retf n (callee pops its arguments)
```
