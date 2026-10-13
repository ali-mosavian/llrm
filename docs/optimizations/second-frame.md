# The second frame

For contributors to the backend's frame layout. Explains why a function may be laid out twice and how the second layout is made without running the backend again.

## Why two layouts

A frame operand past `[bp-128]` needs a four-byte displacement. When a function has a spill area and such operands, `machined_once` ([`assemble.rs`](backend/assemble.md)) also tries a layout with a hole of `spilled` bytes above the allocas for the spill slots, and keeps whichever has fewer far operands (`far_frame`). No hole is tried when the target is basic, the frame is native, there are no spills, or nothing is far.

## How the second layout is made

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef note fill:transparent,stroke:transparent,color:#64748b
  linkStyle default stroke:#64748b,stroke-width:2.2px
  isel["<b>isel + allocation</b><br/><small>cheaper: spiller route /<br/>allocator alone</small>"]:::src
  first["<b>first body</b><br/><small>Frame#123;floor, hole = 0#125;</small>"]:::mid
  rel["<b>relayout::laid_again</b><br/><small>(first, frame, hole)</small>"]:::opt
  note_["moves each cell by its slot:<br/>incoming arguments stay#59;<br/>isel slots move down by hole#59;<br/>spill slots go where Frame::slot<br/>would put them with the hole"]:::note
  second["<b>second body</b>"]:::mid
  pick["<b>far_frame(second) #lt;</b><br/><small>far_frame(first) ?<br/>second : first</small>"]:::out
  isel --> first
  first -->|"far_frame(first) #gt; 0<br/>and spilled #gt; 0"| rel
  rel --> second
  second --> pick
  note_ -.-> rel
```

*The second layout*

[`relayout.rs`](backend/relayout.md) works because every frame cell names the slot it lies in. It gives up, and the first layout stands, on bytes of inline code that name a displacement, a cell in no known slot, or call-private ranges it cannot move. Every pricing and encodability decision treats a frame cell as near, so the two layouts decide alike.

`LLRM_CHECK_FRAME=1` also runs the backend again with the hole and panics if the result differs from the relaid function.

## Not done

Readers that still take a concrete displacement (overlap tests, adjacency arithmetic, encoding sizes, debug homes) could take slot ids and offsets and leave the numbering to one finalisation pass. Today the layout is decided where each cell is made and moved by `relayout`.

See [the backend phases](backend/readme.md) around it.
