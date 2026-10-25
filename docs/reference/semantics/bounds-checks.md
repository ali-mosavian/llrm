# Bounds checks

For users choosing check policy and for frontend authors. A subscript is checked only where the user asked.

## What the user selects

`-fsanitize=bounds` (or `-fsanitize=undefined`; `-fno-sanitize=bounds` turns it off) is the one switch, parsed with the other gcc options in [`flags.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/backend/llrm-core/src/driver/flags.rs#L96). It is off by default. A frontend reads `Flags::sanitize.bounds` and marks its own semantics: `llrm-qb` sets `checked_arrays` ([`cli.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/frontends/qb/src/cli.rs#L108)), which is BC's `/D` subscript check. The two other `/D` checks are separate switches ([integer-overflow-policy](integer-overflow-policy.md)).

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef data fill:#7c3aed,stroke:#5b21b6,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  sw["<b>-fsanitize=bounds</b>"]:::src
  fl["<b>Flags.sanitize.bounds</b>"]:::mid
  fe["<b>llrm-qb</b><br/><small>Frontend.checked_arrays</small>"]:::opt
  sem["<b>qbfront semantic</b><br/><small>Options.checked_arrays</small>"]:::opt
  un["<b>unchecked: subscript = address</b><br/><small>arithmetic from the descriptor</small>"]:::out
  ch["<b>checked: branch to an error</b><br/><small>block (error 9) before it</small>"]:::data
  mir["<b>HIR -#gt; MIR: ordinary</b><br/><small>compares, loads, geps</small>"]:::mid
  sw --> fl
  fl --> fe
  fe --> sem
  sem -->|"off"| un
  sem -->|"on"| ch
  un --> mir
  ch --> mir
```

*From the switch to the check*

## What the frontend emits

qbfront ([`bounds.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/frontends/qb/src/semantic/bounds.rs)) reads a bound from the array descriptor. Where the procedure's own `DIM` or `REDIM` reached the read, the bound is what that statement handed the runtime: the frontend holds each array's rank and bounds in locals, so `promote` and `gvn` fold them, and drops them at `ERASE`, a `DIM`, a call that can reach the array, and any entry the runtime makes. `B$LBND` and `B$UBND` run only to raise "Subscript out of range": always for a dimension past the declared rank, and under `-fsanitize=bounds` where the array may be unallocated or the dimension outside its rank.

Because a check is an ordinary compare and branch in MIR, the optimizer treats it like any other: `ranges` removes it where the index interval proves it, and `unswitch`, `hoist` and `gvn` move or share it.

## Objects compiled by BC

A BC object compiled with `/D` or `/Ah` calls `B$HARY` for each element address. The raise ([`access.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/target/llrm-x86-bc/src/access.rs)) keeps it a call, `@llrm.qb.B$HARY`, answering `{bx, es}`: computing the address in the raise would drop the check the program was compiled with, and a huge element is a 32-bit offset a 16-bit far index cannot hold. The routine's contract in `runtime.toml` says it reads any memory, writes its own, and raises an error, so `gvn` does not share two calls and `hoist` does not move them.

## Not selectable

Checks cannot be added to an object compiled without them: an access may already be inline machine code, and adding a check needs the array's identity, rank and extents.
