# Mermaid figures: trial

Audience: the maintainers choosing how docs draw figures. GitHub renders a `mermaid` fence in markdown, sized to the page, in its light and dark themes, with no generated file. This page draws our figures that way with one shared bold style; compare with the generated PNG figures in #1546.

## Style

Every figure starts with the same `init` line (font, sizes, neutral line and caption colours that read on the light and the dark theme, smooth edges) and the same classes, so a figure names meaning, never colour: solid saturated fills with white text, a darker outline, large corners, thick arrows, and groups tinted in the colour of what they hold.

| Class | Meaning | Colour |
|---|---|---|
| `src` | input, a front end, a source | blue |
| `mid` | an ordinary stage or value | slate |
| `opt` | the optimizer, the thing being explained | orange |
| `out` | output, a written file | green |
| `data` | a table, a description, generated input | violet |
| `warn` | refused, an error, a hazard | red |
| `note` | free text beside a figure, no box | none |

## Pipeline

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef data fill:#7c3aed,stroke:#5b21b6,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef warn fill:#dc2626,stroke:#991b1b,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef note fill:transparent,stroke:transparent,color:#64748b
  linkStyle default stroke:#64748b,stroke-width:2.2px
  subgraph front["SOURCE FRONT ENDS"]
    direction LR
    qb["<b>① QuickBASIC</b><br/><small>llrm-qb</small>"]:::src
    nib["<b>① Nib</b><br/><small>llrm-nib</small>"]:::src
    c["<b>① C</b><br/><small>llrm-c</small>"]:::src
  end
  qb --> hir
  nib --> hir
  c --> hir
  hir["<b>② HIR</b><br/><small>typed, per language</small>"]:::mid -->|mir::emit| mir["<b>③ MIR</b><br/><small>SSA, target-neutral</small>"]:::mid
  mir --> opt["<b>④ Optimizer</b><br/><small>llrm-transforms</small>"]:::opt
  opt ==>|lowering boundary| back
  subgraph back["BACKEND"]
    sel["<b>⑤ Instruction selection</b><br/><small>LIR over virtual values</small>"]:::mid --> ph["<b>⑥ Machine phases</b><br/><small>register allocation, peephole</small>"]:::mid --> obj["<b>⑦ Object writer</b><br/><small>OMF, ELF, COFF, Mach-O</small>"]:::out
  end
  style front fill:#2563eb12,stroke:#2563eb,stroke-width:1.6px,stroke-dasharray:7 5,color:#2563eb
  style back fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
```

*From source to object file.*

## Frame of a QuickBASIC procedure

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','textColor':'#64748b'}}}%%
block-beta
  columns 8
  space:1 up["▲ higher addresses"]:3 space:4
  o1["bp+8"] c1["formal 1"]:3 s1["2"] n1[" "]:3
  o2["bp+6"] c2["formal 2"]:3 s2["2"] n2["last formal, nearest the return address"]:3
  o3["bp+2"] c3["return address"]:3 s3["4"] n3["far: offset, then segment"]:3
  o4["bp"] c4["runtime header"]:3 s4["10"] n4["saved BP, frame chain, SI, DI (QB 4.5)"]:3
  o5[" "] c5["locals and spills"]:3 s5[" "] n5["SP is at the bottom"]:3
  space:1 dn["▼ stack grows down"]:3 space:4
  classDef off fill:transparent,stroke:transparent,color:#64748b
  classDef cell fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff
  classDef ret fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff
  classDef hdr fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff
  classDef size fill:#1e293b,stroke:#0f172a,stroke-width:2px,color:#e2e8f0
  class up,dn,o1,o2,o3,o4,o5,n1,n2,n3,n4,n5 off
  class c1,c2,c5 cell
  class c3 ret
  class c4 hdr
  class s1,s2,s3,s4,s5 size
```

*Pushed by the caller: the formals and the return address. B$ENRA builds the header.*

## Borrow timeline

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','textColor':'#64748b','titleColor':'#64748b','gridColor':'#94a3b8','sectionBkgColor':'transparent','sectionBkgColor2':'transparent','altSectionBkgColor':'transparent','taskTextColor':'#ffffff','taskTextOutsideColor':'#64748b','taskTextLightColor':'#ffffff','taskTextDarkColor':'#ffffff','doneTaskBkgColor':'#16a34a','doneTaskBorderColor':'#166534','activeTaskBkgColor':'#2563eb','activeTaskBorderColor':'#1e40af','critBkgColor':'#dc2626','critBorderColor':'#991b1b','todayLineColor':'transparent'},'gantt':{'barHeight':34,'barGap':12,'topPadding':48,'leftPadding':150,'fontSize':14,'sectionFontSize':14,'numberSectionStyles':2}}}%%
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
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','actorBkg':'#2563eb','actorBorder':'#1e40af','actorTextColor':'#ffffff','actorLineColor':'#94a3b8','signalColor':'#64748b','signalTextColor':'#64748b','noteBkgColor':'#ea580c','noteBorderColor':'#9a3412','noteTextColor':'#ffffff','activationBkgColor':'#cbd5e1','activationBorderColor':'#64748b','lineColor':'#64748b'},'sequence':{'mirrorActors':false,'messageAlign':'center','actorMargin':70,'boxMargin':10,'width':170,'height':46,'noteMargin':12}}}%%
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
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef data fill:#7c3aed,stroke:#5b21b6,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef warn fill:#dc2626,stroke:#991b1b,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef note fill:transparent,stroke:transparent,color:#64748b
  linkStyle default stroke:#64748b,stroke-width:2.2px
  subgraph fe["FRONT ENDS"]
    direction TB
    qb["<b>llrm-qb</b><br/><small>QuickBASIC</small>"]:::src
    nb["<b>llrm-nib</b><br/><small>Nib</small>"]:::src
    cc["<b>llrm-c</b><br/><small>C, Open Watcom</small>"]:::src
  end
  subgraph rep["REPRESENTATIONS"]
    direction TB
    hir["<b>llrm-hir</b><br/><small>typed HIR</small>"]:::mid
    mir["<b>llrm-mir</b><br/><small>SSA MIR</small>"]:::mid
    lir["<b>llrm-lir</b><br/><small>machine form</small>"]:::mid
  end
  subgraph mid["MIDDLE AND BACK END"]
    direction TB
    tr["<b>llrm-transforms</b><br/><small>the optimizer</small>"]:::opt
    an["<b>llrm-analysis</b><br/><small>shared facts</small>"]:::opt
    core["<b>llrm-core</b><br/><small>isel, allocation, layout</small>"]:::mid
  end
  subgraph tg["TARGETS AND OBJECTS"]
    direction TB
    t16["<b>llrm-x86-m16</b><br/><small>real mode</small>"]:::data
    t32["<b>llrm-x86-m32</b><br/><small>flat</small>"]:::data
    obj["<b>llrm-object</b><br/><small>omf, coff, elf, macho</small>"]:::out
  end
  fe ==> rep ==> mid ==> tg
  style fe fill:#2563eb12,stroke:#2563eb,stroke-width:1.6px,stroke-dasharray:7 5,color:#2563eb
  style rep fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
  style mid fill:#ea580c12,stroke:#ea580c,stroke-width:1.6px,stroke-dasharray:7 5,color:#ea580c
  style tg fill:#7c3aed12,stroke:#7c3aed,stroke-width:1.6px,stroke-dasharray:7 5,color:#7c3aed
```

*An arrow reads "feeds"; the real edges are in [crates](../reference/crates.md).*
