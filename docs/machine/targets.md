# Targets

For contributors adding or changing a target. A target is a set of description files, a data crate that holds them and a select crate that holds the selector generated from them. No shared pass or IR names a target.

## Targets built

| Name | Crates | Platform |
|---|---|---|
| `x86-m16` (default) | `llrm-x86-m16`, `llrm-x86-m16-select` | 16-bit real mode, far calls, selectors, huge pointers; 386+ instructions under prefixes |
| `x86-m32` | `llrm-x86-m32`, `llrm-x86-m32-select` | flat 32-bit (CS = DS = SS, base 0), DOS under an extender |

`-m16` and `-m32` choose between them; `llrm_driver::all()` lists them.

## Crate graph

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  drv["<b>llrm-driver</b><br/><small>the one crate that<br/>names targets</small>"]:::src
  core["<b>llrm-codegen (backend)</b><br/><small>depends on no target crate</small>"]:::opt
  s16["<b>llrm-x86-m16-select</b><br/><small>SELECT crate: build.rs<br/>runs llrm-iselgen</small>"]:::mid
  s32["<b>llrm-x86-m32-select</b><br/><small>SELECT crate: build.rs<br/>runs llrm-iselgen</small>"]:::mid
  d16["<b>llrm-x86-m16</b><br/><small>DATA crate: descriptions<br/>+ Target impl#59; no llrm-codegen</small>"]:::mid
  d32["<b>llrm-x86-m32</b><br/><small>DATA crate: descriptions<br/>+ Target impl#59; no llrm-codegen</small>"]:::mid
  x86["<b>llrm-x86</b><br/><small>form schema and parser,<br/>encoder, x87 and i64 hooks,<br/>family.isel, helpers</small>"]:::mid
  tgt["<b>llrm-target</b><br/><small>Target trait, layout,<br/>registers, calling, timings,<br/>opcosts, object, OS</small>"]:::mid
  subgraph g_irs["IRS#59; NAME NO TARGET"]
    lir["<b>llrm-lir</b>"]:::mid
    mir["<b>llrm-mir</b>"]:::mid
  end
  drv --> s16
  drv --> s32
  drv --> d16
  drv --> d32
  drv --> core
  s16 --> core
  s32 --> core
  s16 --> x86
  s32 --> x86
  d16 --> x86
  d32 --> x86
  x86 --> tgt
  core --> tgt
  tgt --> lir
  tgt --> mir
  style g_irs fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
```

*Crate graph of the targets; an arrow reads "depends on"*

A data crate does not depend on `llrm-codegen`; a select crate does, and only `llrm-driver` depends on a select crate. `llrm-codegen` has no target among its normal dependencies (`tests/core_depends_on_no_target.rs`). Its own unit tests need a selector, so a `fixtures` feature generates the m16 and m32 selectors inside `llrm-codegen` for them, and nothing else turns it on (`tests/core_fixtures_are_for_core_tests.rs`). The data/select split is held by a manifest test proposed in [#1273](https://github.com/ali-mosavian/llrm/pull/1273).

## What a target ships

Files under `crates/target/llrm-x86-m32/src` (m16 has the same set):

| File | Says |
|---|---|
| `registers.regs` | registers, subregisters, classes, allocation order, DWARF and CodeView numbers |
| `instructions/x86.instr` | forms: operands, widths, ties, fixed registers, flags, cost key, encoding |
| `isel/patterns.isel` | MIR shape to forms; the generator turns them into a decision automaton |
| `isel/peephole.peep` | form windows to forms |
| `timings.times` | a column per CPU: clocks, latency, issue, prefix and stall costs |
| `opcosts.txt` | each MIR operation priced as a sum of form prices; `unroll_budget` |
| `machines/datalayout.toml` | data layout, address-space kinds, address forms; the `f80` entry is the x87 `long double`'s alignment, so its size in memory (`Layout::long_double_bytes`: 10 at m16, 12 at m32; the front end is built with it, so `sizeof`, arrays and members of a `long double` lay out at that size) |
| `machines/calling.toml` | conventions: slot size, argument registers, results, cleanup, preserved |
| `machines/object.toml` | object writer and listing syntax |
| `machine/platform/dos/real16.toml`, `flat32.toml`, `ports.toml` | the platform: memory map, ports (apart from the target) |

The family crate `llrm-x86` holds the forms and peephole rules every x86 target shares (`x86.instr`, `family.isel`, `peephole.peep`) and `helpers/*.asm`. `llrm-target` holds the parsers and the `Target` trait the passes query ([`lib.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/target/llrm-target/src/lib.rs#L210)): layout and address spaces, the frame register and where arguments lie, results, callee-saved registers, CPUs and `-march` names, operation costs per CPU, address forms, float stack depth.

## From description to compiler

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  src["<b>Description files</b><br/><small>*.regs *.instr *.isel *.peep<br/>timings.times opcosts.txt<br/>*.toml</small>"]:::src
  gen["<b>llrm-iselgen</b><br/><small>(build.rs of the select crate)</small>"]:::mid
  out["<b>SELECTOR, RULES,</b><br/><small>effect rows</small>"]:::opt
  impl["<b>Target impl (data crate)</b><br/><small>read by the passes</small>"]:::opt
  drv["<b>llrm-driver binds</b>"]:::mid
  core["<b>llrm-codegen backend</b>"]:::out
  src --> gen
  gen --> out
  src --> impl
  out --> drv
  impl --> drv
  drv --> core
```

*From target description to compiler*

A pattern names a hook (`call NAME`, `when NAME`): a name `llrm-codegen` exports becomes a call on its selection context; any other resolves to the target crate's hook function, and a missing one is a compile error. Cost expressions, loops and searches are hooks, not description.

## Adding a target

1. A data crate and a select crate in `crates/target`, named `llrm-<isa>-<mode>` and `llrm-<isa>-<mode>-select`, with the files above and a `Target` impl.
2. A `build.rs` in the select crate that calls `llrm_iselgen::build::target` with the family directory and `llrm-codegen`'s `peep/groups.list`.
3. One arm in `llrm_driver::all` and `llrm_driver::selector`; nothing else matches on a name.
4. A pass or IR that needs something the target does not state takes it as a parameter from the `Target` trait. An enum variant, constant or branch for one target in a shared crate is a defect.

## Axes

Mode (m16, m32) sets the default operand and address size, segmentation, pointer width, conventions and object format. The CPU (`-march`, `-mtune`; the columns of `timings.times`) sets which instructions and forms exist and their prices. Address forms are per (mode, address size); m16 describes both the 16-bit forms and the 32-bit ones behind `67h` ([a32](a32.md)). m16 keeps its 386 code generation (`test_m16_emits_386_forms`).
