> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/isel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/isel.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/isel.rs sha256:6b4b6985330ce32a723be17c737fe5340d3a16ec9b7029466d121feb610e3132 -->

# isel

Instruction selection: one MIR function becomes LIR over virtual registers,
as LLVM's SelectionDAG makes MachineInstrs.

## How it works
Selection chooses instructions and addressing. Two-address form, registers
and the frame's size are left to the machine phases. The rules that choose
are data, not code in this file: they are generated at build time and run by
`isel/matcher.rs`.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  s["<b>family.isel +</b><br/><small>target patterns.isel +<br/>x86.instr</small>"]:::mid
  c["<b>Compiled: automaton states,</b><br/><small>holds, cost, emit<br/>(one per target)</small>"]:::mid
  m["<b>MIR function</b>"]:::mid
  bd["<b>Selector::body: frame</b><br/><small>objects, edges,<br/>entry instructions</small>"]:::mid
  bl["<b>blocks of LIR, phis,</b><br/><small>terminators</small>"]:::mid
  l["<b>LirBody</b>"]:::mid
  s -->|"llrm-iselgen"| c
  m -->|"selected_with: analyses,<br/>convention, parameters"| bd
  bd -->|"each instruction, in reverse<br/>postorder: features, automaton,<br/>candidates, holds/cost, emit"| bl
  bl -->|"unread halves dropped,<br/>combined, widen,<br/>landing-pad root"| l
```

*Instruction selection: build time (build.rs, top), run time per function
(bottom)*

### Rules from generated definitions
A target's `patterns.isel` is spliced into the x86 family's file, and
`llrm-iselgen` checks every `emit` against the forms in `x86.instr`. Each
pattern has one `match` line (opcode, result type, operand kinds and types,
nested defining instructions), optional `when` predicates, `let` operand
constructors, and a body: `emit` a form, `call` a hook written by hand
(`hook_` in `matcher.rs`), `chain` an operation on a wide integer as its
halves, `refuse` or `nothing`. `group` members with a `cost` compete; a
pattern with `cover` takes over a nested instruction. The generator turns
the `match` lines into a decision automaton, as TableGen merges common
prefixes, and the rest into `holds`, `cost` and `emit` functions.

### Matching
For an instruction the matcher computes its features (opcode, result type,
and each operand's kind and type, up to three operands; a constant first
operand of a commutative opcode is swapped second) and walks the automaton
to a leaf: the patterns still standing, in file order. The first whose
`when` predicates hold is chosen; if it is in a group, the cheapest holding
member of the group is. Its `emit` appends LIR. A shape no pattern takes is
refused, by the opcode's name.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  i["<b>add i16 %a, 7</b>"]:::mid
  f["<b>features</b><br/><small>[add, i16, value, i16, int,<br/>i16, none, none ...]</small>"]:::mid
  l["<b>leaf</b><br/><small>[selected_elsewhere,<br/>arithmetic_promoted,<br/>arithmetic]</small>"]:::mid
  e["<b>arithmetic</b><br/><small>emit add result #lt;- x, y<br/>x = held(a), y = source(7)</small>"]:::mid
  i --> f
  f -->|"automaton: opcode? -#gt;<br/>type? -#gt; operand kinds? ..."| l
  l -->|"holds(): first that holds<br/>(when promoted may fail)"| e
```

*Selecting add i16 %a, 7*

Decisions that need more than the instruction are made first, per
instruction, before any is emitted: a comparison read only by its block's
branch stays flags (`fuse`); a dword load read only through `trunc` is two
word loads (`narrowed`); a quotient and a remainder of the same operands are
one division (`pair`); the first covering pattern that holds marks what it
covers (`covered_by_pattern`), and that instruction is then selected by the
cover alone.

### What the hooks cover
What is not pattern-shaped is in this file: calls (the convention decides
what is pushed, passed in registers and popped, `called`), `memcpy` and
`memset` (by pieces or `rep`), float conversions and compares, far pointers
as an offset and a selector (`fars`), address arithmetic folded into memory
operands (`indexed`), `switch`, and branch conditions merged into one chain.
Allocas get frame cells (`alloca_groups`, `Selected::extents`); a parameter
arrives in a cell or in registers, delivered by an entry instruction.

### Wide integers
An integer wider than the target's native one is two halves, low first
(`wide.rs`, `expand.rs`): `chain` patterns for add, sub and the bit
operations, hooks for the rest. A value's sign bits can show that a product
or quotient of 32-bit values needs one instruction.

```text
 add i64 %a, %b  (32-bit target)
   add  lo <- a.lo, b.lo
   adc  hi <- a.hi, b.hi        takes the carry of the half below
```

### After the instructions
Wide halves nothing reads are dropped. `combined` then makes the selections
that need neighbours; `widen` marks exact address sums and makes a word
value used as a dword address component a dword one
(`addressforms::promote`); if the function has a landing pad, `rooted` adds
a root block that reaches it. Block frequencies and branch odds are set from
the MIR's.

## Example
Before, the loop of `Acc.run` inlined into `main` (the MIR `-O2` leaves):

```mir
2:
  %lsr.iv1 = phi i16 [ -7, %b1 ], [ %lsr.iv.next, %2 ]
  %3 = load i16, ptr addrspace(5) %1
  %4 = add i16 %3, 3
  store i16 %4, ptr addrspace(5) %1
  %lsr.iv.next = add i16 %lsr.iv1, 1
  %5 = icmp ne i16 %lsr.iv.next, 0
  br i1 %5, label %2, label %6
```

After, the block as the first machine phase receives it:

```lir
    block 0x0004
      v5 := phi 0x0000:v1 0x0004:v4
      ; ...
      0x0007  [[abs+0x0] v2@unplaced] := add [[abs+0x0] v2@unplaced], 0x3  ; inserted
      0x0008  v4 := add v5, 0x1  ; inserted
      0x000a  cmp v4, 0x0  ; inserted
      0x000a  jne   ; inserted
```

The load, add and store of one cell are one `add` on memory, and the
compare and branch are `cmp` and `jne`; registers are still virtual.

From `ISEL_DUMP=1` of a nib program (a struct method with a `while` loop,
called from `main`) at `-m16 -O2`, stage `FarIndirectCalls`.

## Runs
First of the staged phases in `assemble::staged`, once per function and
again at -Omax where a callee's known register effects were used (the second
run selects without them and the cheaper is kept). Its output is the input
of `framefree` and `frame::of`.

## Needs
`llrm_analysis` (CFG shape, memory, constants, ranges) for what the
patterns' predicates ask; [`crate::backend::calleefacts`](calleefacts.md) for the registers
a callee writes; the target's [`crate::backend::isel::Compiled`](isel.md).

## See also
`docs/machine/targets.md`; LLVM `SelectionDAGISel`.
