# Mermaid figures: trial

Audience: the maintainers choosing how docs draw figures. GitHub renders a `mermaid` fence in markdown, sized to the page, in its light and dark themes, with no generated file. This page draws our figures that way with one shared style; compare with the generated PNG figures in #1546.

## Style

Every figure starts with the same `init` line (font, sizes, neutral colours that read on the light and the dark theme, transparent group and label backgrounds, smooth edges) and the same classes, so a figure only names meaning, never colour:

| Class | Meaning | Fill |
|---|---|---|
| `src` | input, a front end, a source | blue |
| `mid` | an ordinary stage or value | grey |
| `opt` | the optimizer, the thing being explained | orange |
| `out` | output, a written file | green |
| `data` | a table, a description, generated input | purple |
| `warn` | refused, an error, a hazard | red |
| `note` | free text beside a figure, no box | none |

## Pipeline

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'14px','primaryColor':'#eef2f7','primaryTextColor':'#1f2328','primaryBorderColor':'#8b98a8','lineColor':'#7d8590','secondaryColor':'#eef2f7','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#8b98a8','edgeLabelBackground':'#f6f8fa','textColor':'#7d8590','titleColor':'#7d8590'},'flowchart':{'curve':'basis','nodeSpacing':34,'rankSpacing':46,'padding':14,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src  fill:#dbeafe,stroke:#2563eb,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef mid  fill:#eef2f7,stroke:#8b98a8,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef opt  fill:#ffedd5,stroke:#c2410c,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef out  fill:#dcfce7,stroke:#15803d,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef data fill:#f3e8ff,stroke:#7e22ce,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef warn fill:#fee2e2,stroke:#b91c1c,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef note fill:transparent,stroke:transparent,color:#7d8590
  linkStyle default stroke:#7d8590,stroke-width:1.6px
  subgraph front["Source front ends"]
    direction LR
    qb["<b>QuickBASIC</b><br/><small>llrm-qb</small>"]:::src
    nib["<b>Nib</b><br/><small>llrm-nib</small>"]:::src
    c["<b>C</b><br/><small>llrm-c</small>"]:::src
  end
  qb --> hir
  nib --> hir
  c --> hir
  hir["<b>HIR</b><br/><small>llrm-hir</small>"]:::mid -->|mir::emit| mir["<b>MIR</b><br/><small>llrm-mir</small>"]:::mid
  mir --> opt["<b>Optimizer</b><br/><small>llrm-transforms</small>"]:::opt
  opt -->|lowering boundary| back
  subgraph back["Backend"]
    sel["<b>Instruction selection</b><br/><small>LIR over virtual values</small>"]:::mid --> ph["<b>Machine phases</b><br/><small>register allocation, peephole</small>"]:::mid --> obj["<b>Object writer</b><br/><small>OMF, ELF, COFF, Mach-O</small>"]:::out
  end
  style front stroke:#8b98a8,stroke-width:1.4px,stroke-dasharray:6 4,fill:transparent,color:#7d8590
  style back stroke:#8b98a8,stroke-width:1.4px,stroke-dasharray:6 4,fill:transparent,color:#7d8590
```

*From source to object file.*

## Frame of a QuickBASIC procedure

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'14px','primaryColor':'#eef2f7','primaryTextColor':'#1f2328','primaryBorderColor':'#8b98a8','lineColor':'#7d8590','textColor':'#7d8590'}}}%%
block-beta
  columns 8
  space:1 up["higher addresses ↑"]:3 space:4
  o1["bp+8"] c1["formal 1"]:3 s1["2"] n1[" "]:3
  o2["bp+6"] c2["formal 2"]:3 s2["2"] n2["last formal, nearest the return address"]:3
  o3["bp+2"] c3["return address"]:3 s3["4"] n3["far: offset, then segment"]:3
  o4["bp"] c4["runtime header"]:3 s4["10"] n4["saved BP, frame chain, SI, DI (QB 4.5)"]:3
  o5[" "] c5["locals and spills"]:3 s5[" "] n5["SP is at the bottom"]:3
  space:1 dn["stack grows down ↓"]:3 space:4
  classDef off fill:transparent,stroke:transparent,color:#7d8590
  classDef cell fill:#eef2f7,stroke:#8b98a8,stroke-width:1.6px,color:#1f2328
  classDef ret fill:#dbeafe,stroke:#2563eb,stroke-width:1.6px,color:#1f2328
  classDef hdr fill:#ffedd5,stroke:#c2410c,stroke-width:1.6px,color:#1f2328
  classDef size fill:#eef2f7,stroke:#8b98a8,stroke-width:1.6px,color:#7d8590
  class up,dn,o1,o2,o3,o4,o5,n1,n2,n3,n4,n5 off
  class c1,c2,c5 cell
  class c3 ret
  class c4 hdr
  class s1,s2,s3,s4,s5 size
```

*Pushed by the caller: the formals and the return address. B$ENRA builds the header.*

## Borrow timeline

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'14px','textColor':'#7d8590','titleColor':'#7d8590','gridColor':'#8b98a8','sectionBkgColor':'transparent','sectionBkgColor2':'transparent','altSectionBkgColor':'transparent','taskTextColor':'#1f2328','taskTextOutsideColor':'#7d8590','taskTextDarkColor':'#1f2328','doneTaskBkgColor':'#dcfce7','doneTaskBorderColor':'#15803d','activeTaskBkgColor':'#dbeafe','activeTaskBorderColor':'#2563eb','critBkgColor':'#fee2e2','critBorderColor':'#b91c1c','todayLineColor':'transparent'},'gantt':{'barHeight':30,'barGap':10,'topPadding':44,'leftPadding':150,'fontSize':13,'sectionFontSize':13,'numberSectionStyles':2}}}%%
gantt
  dateFormat X
  axisFormat %s
  tickInterval 1
  section v (owner)
  owner of v                        :done, v, 1, 9
  section r = &v[0]
  shared borrow, live to its last use :active, r, 3, 9
  v.push(3) is refused              :crit, c, 6, 7
```

*Line numbers on the axis: 1 `let mut v`, 3 `let r = &v[0]`, 6 `v.push(3)`, 9 `print(r)`.*

## Call and return

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'14px','actorBkg':'#dbeafe','actorBorder':'#2563eb','actorTextColor':'#1f2328','actorLineColor':'#8b98a8','signalColor':'#7d8590','signalTextColor':'#7d8590','noteBkgColor':'#ffedd5','noteBorderColor':'#c2410c','noteTextColor':'#1f2328','activationBkgColor':'#eef2f7','activationBorderColor':'#8b98a8'},'sequence':{'mirrorActors':false,'messageAlign':'center','actorMargin':60,'boxMargin':8}}}%%
sequenceDiagram
  participant C as caller
  participant F as callee
  C->>C: push the arguments (Pascal order)
  C->>F: call far (pushes CS:IP)
  activate F
  F->>F: push bp · mov bp,sp · sub sp,locals
  Note over F: the runtime header is built by B$ENRA
  F-->>C: retf n (the callee pops its arguments)
  deactivate F
```

## Crate layering

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'14px','primaryColor':'#eef2f7','primaryTextColor':'#1f2328','primaryBorderColor':'#8b98a8','lineColor':'#7d8590','secondaryColor':'#eef2f7','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#8b98a8','edgeLabelBackground':'#f6f8fa','textColor':'#7d8590','titleColor':'#7d8590'},'flowchart':{'curve':'basis','nodeSpacing':34,'rankSpacing':46,'padding':14,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef src  fill:#dbeafe,stroke:#2563eb,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef mid  fill:#eef2f7,stroke:#8b98a8,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef opt  fill:#ffedd5,stroke:#c2410c,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef out  fill:#dcfce7,stroke:#15803d,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef data fill:#f3e8ff,stroke:#7e22ce,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef warn fill:#fee2e2,stroke:#b91c1c,stroke-width:1.6px,color:#1f2328,rx:9px,ry:9px
  classDef note fill:transparent,stroke:transparent,color:#7d8590
  linkStyle default stroke:#7d8590,stroke-width:1.6px
  subgraph fe["Front ends"]
    direction TB
    qb["<b>llrm-qb</b>"]:::src
    nb["<b>llrm-nib</b>"]:::src
    cc["<b>llrm-c</b>"]:::src
  end
  subgraph rep["Representations"]
    direction TB
    hir["<b>llrm-hir</b>"]:::mid
    mir["<b>llrm-mir</b>"]:::mid
    lir["<b>llrm-lir</b>"]:::mid
  end
  subgraph mid["Middle and back end"]
    direction TB
    tr["<b>llrm-transforms</b><br/><small>optimizer</small>"]:::opt
    an["<b>llrm-analysis</b>"]:::opt
    core["<b>llrm-core</b><br/><small>isel, allocation, layout</small>"]:::mid
  end
  subgraph tg["Targets and objects"]
    direction TB
    t16["<b>llrm-x86-m16</b>"]:::data
    t32["<b>llrm-x86-m32</b>"]:::data
    obj["<b>llrm-object</b> → omf, coff, elf, macho"]:::out
  end
  fe --> rep --> mid --> tg
  style fe stroke:#8b98a8,stroke-width:1.4px,stroke-dasharray:6 4,fill:transparent,color:#7d8590
  style rep stroke:#8b98a8,stroke-width:1.4px,stroke-dasharray:6 4,fill:transparent,color:#7d8590
  style mid stroke:#8b98a8,stroke-width:1.4px,stroke-dasharray:6 4,fill:transparent,color:#7d8590
  style tg stroke:#8b98a8,stroke-width:1.4px,stroke-dasharray:6 4,fill:transparent,color:#7d8590
```

*An arrow reads "feeds"; the real edges are in [crates](../reference/crates.md).*
