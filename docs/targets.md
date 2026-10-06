# Targets

llrm builds code for one target today, x86 real mode (`x86-m16`). This
document is the plan to make a target a crate plus descriptions: the inventory of
what the shared code assumes, the interface that replaces the assumptions, what
each crate owns, and the PR sequence. Optimization targets are in
[measurement/targets.md](measurement/targets.md).

Targets the design must fit: `x86-m16`, `x86-m32` (flat), `x86-m64`,
`arm64`. Only m32 is built in this task.

## First principle

**Adding a target never customises an IR (HIR, MIR, LIR) or a shared pass.** The
target's descriptions and the machine model are the only specifics; everything else
stays as it is. No enum variant, constant, default or branch for one target in an IR
crate or a shared pass: what differs between targets reaches the pass as a parameter
the driver (the one place that names targets) hands in. Review every PR, this
plan's and m32's, against it.

Known departures today, each to be removed:

| Where | What | Removed by |
|---|---|---|
| `llrm-core`: the `target.rs` statics, `select.rs` bases, `masm`/`sharedstores` register lists | m16's registers read directly | PR 19 (a register-class description the allocator receives) |
| `llrm-hir`, `llrm-mir`: the datalayout string, address-space numbers, `TargetProfile` variants | a target's layout in an IR crate | PR 14a, and the m32 session's HIR change (data layout and address spaces from the `Target`) |
| `llrm-core` `select.rs`, `objbuild.rs`: x86 instruction encoding and the layout of an object, in `llrm-core` and keyed by a described bitness (`At{ip,bits}`) | the encoder belongs in the x86 family layer | PR 5's encoder half (m32-prep, D) |
| `llrm-core` `select.rs` encoder and `peephole::_register_effects`: x86 encoding, and register effects read by decoding the emitted bytes, in `llrm-core` | the encoder and the effects belong in the x86 family layer; the mode they run in is the body's (`LirBody::bits`, set by isel from the target's `object.bitness`) | the encoder PR (m32-prep, D); #581 |

## Principle: a target is description

Adding a target is writing description files, as LLVM's `.td` files are. Custom
Rust is the exception: a **named hook** in the target crate for what the
language cannot say, never a branch on the target in shared code.

A target crate ships, and generic code reads:

| File | Format | Says |
|---|---|---|
| registers | line table | registers, subregisters, classes, kind letters (`r` is gpr, `s` is x87), allocation order, reserved |
| forms | line table (`x86.instr`) | operands, widths, ties, fixed registers, flags read and written, cost key, opaque `encoding` |
| patterns, peephole | `patterns.isel`, `peephole.peep` | MIR shape to forms; form windows to forms |
| timings | `timings.times`: a column per CPU, scalars and `[clocks]`/`[latency]` tables | clocks, latencies, issue and prefix costs; read by `llrm-target` |
| operation costs | `opcosts.txt`: each MIR operation as a sum of form prices, a `[bytes]` table and `[size]` overrides | how the passes' prices are made of the forms' |
| datalayout | TOML | datalayout string, address-space kinds (linear, pair, selector), legal types, address-form table (base and index class, scales, displacement range per width) |
| calling conventions | TOML | slot size, order, cleanup, argument registers, result by type class, preserved |
| object format | TOML | writer and listing syntax |

A machine's memory map and ports (`dos.toml`) describe a platform, not a
target, and stay apart.

### Hooks

For `call NAME`, `when NAME`, `let x = NAME(..)` the generator resolves the name
at generation time: a name in the list `llrm-core` exports becomes a call on
its `SelectCx`; any other becomes `crate::isel::hooks::NAME` in the target
crate, one fixed signature per kind. A missing hook is a compile error. No
registry, no strings, no trait of 26 methods. Family hooks (x87, i64 halves)
live in `llrm-x86` and are re-exported.

### The language stays small

Added, in order of code deleted per feature: (i) match on the callee and on
intrinsics, (ii) condition codes as data with `j{cc}`/`set{cc}` taking a bound
one, (iii) multi-result emit with `partner(..)`, (iv) half constructors `low()`
and `high()` over a wide value, (v) `temp(bytes)` with a value alias.

Schema edits for a non-x86 ISA, four only: kind letters map to register classes;
named immediate kinds checked by a predicate (`i:imm12`); `{cc}`, registers and
widths read from the description, not from `parse.rs` constants, and the `far`
type class becomes the pair kind of an address space; the `iced` column becomes
an opaque `encoding`. `^N` ties, `rm` kinds and `fixed` stay; a target that has
none never writes them.

Not in the language: loops, arithmetic on constants, if/else beyond type
matching, cost expressions, user functions. If the Rust body has a loop or a
search, it is a hook. `peephole.peep` is at that edge: its syntax is frozen.

### The 26 selection hooks

Mapped to phases in "MIR to LIR" below: which become legalizer rules, which
patterns or complex patterns, which stay named hooks. About half the steps are
generic lowering or loops and stay Rust, behind the calling-convention and
address-form descriptions.

## Requirements on m16

1. **Nothing is dropped from the 16-bit backend.** Far calls and returns,
   selectors and the selector register file, huge pointers, DGROUP, the
   interrupt frame, the 16-bit address forms, every CPU profile (386 to Core), every
   peephole rule and pattern, the QB, Nib and BC runtimes stay. A stage moves code
   to where a description or another crate owns it; it does not delete a
   capability. Where a pass is keyed to "has segments", m16 has them.
2. **It produces exactly the code it produces today.** Byte-identical `.obj`
   and listings, not "equivalent": the gate on every PR.
3. **m16 keeps its 386+ code generation** (next section).

## Axes: mode and CPU profile

Two axes, not one.

- **Mode** (m16, m32, m64) sets the default operand and address size,
  segmentation, pointer width, calling conventions and object format.
- **CPU profile** (8086, 286, 386, 486, P5, ...) sets which instructions,
  registers and address forms exist, and their timings. The register
  description has EAX..EDI in m16 too, gated by profile. Encodings are
  shared; the prefixes (66h, 67h) follow from mode.
- Legality is per (mode, profile). Address forms are per (mode, address size):
  the 16-bit forms (`{BX,BP}+{SI,DI}+disp16`) and the 32-bit forms (any GPR
  base, any but ESP an index, scale 1/2/4/8, disp32; behind 67h in m16) are
  both described, each with its cost.

**Requirement: m16 keeps its 386+ code generation.** m16 is 386 code under
prefixes today (32-bit registers and arithmetic, `[ebx+eax*2]`, `movsx`/`movzx`,
`rep movsd`, `shld`; 486 is the default profile). `test_m16_emits_386_forms`
(#502) pins it; no refactor may replace it with an "equivalent" narrowing.
There is no 286 profile yet: `machine::CPUS` starts at 386 and the listing
header is `.386`. A pre-386 profile would be legalizer rules (i32 narrowed to
i16 pairs), not a refactor, and is not part of this task.

`Target` is the mode; the CPU profile is its `CostModel` and register gating
(LLVM's subtarget). `Profile::target()` today builds both as one.

## Target facts, in one line each

- **m16**: real mode. Segments, selectors, far calls, 16-bit addressing
  (`bx/bp/si/di`), dword ops under a 66h prefix.
- **m32**: CS = DS = SS, flat, base 0, 4 GB. One pointer type, `p:32:32`. No
  selectors. ES = DS is a fact the description states, not an assumption: string
  ops read it or set ES. FS and GS are never assumed (TLS). Any GPR is a base,
  any but ESP an index. Word ops pay 66h.

m32 runs under a DOS extender: OMF with USE32 segments and 32-bit records, linked by
JWlink as an LE executable behind DOS/32A's stub (`object.toml`'s `[link]`; the start-up
and `report` are `llrm-x86-m32/runtime`). Its C ABI is cdecl32,
`calling.toml`: Open Watcom's 386 flat ABI as `wccq -ecc -zp4` has it. Arguments are
pushed right to left in dword slots and the caller removes them; symbols are `_name`;
EBX, ESI, EDI and EBP are preserved, the direction flag is clear and the x87 stack empty
on entry and return; results are in EAX, EDX:EAX for 64 bits and ST0 for floats; structs
are packed to 4 bytes. One deliberate difference: a struct return of other than 1, 2 or 4
bytes goes through a pointer pushed after the arguments and returned in EAX (reentrant),
where Open Watcom's `-ecc` returns the address of one static area. A call into an Open
Watcom object that returns such a struct is the one call that does not match.

## Measured state

- `llrm_x86_m16` is named on 65 lines outside its crate: 20 in production
  code, 45 in tests. The MIR crates use it in tests only, and already take it as a
  dev-dependency.
- `llrm_mir::target::Machine` and `DataLayout` are the seam MIR passes read.
  Those passes name no register; what remains in them is address-space numbers
  and 16-bit literals. `Machine` itself carries m16 notions
  (`far_access_registers`, `segment_registers`, `foreign_span(selectors, ..)`,
  `huge_window`, `OperationCosts.copy` as `rep movs`/ES, `carry`).
- `llrm-c` does not use clang. It runs Open Watcom's 16-bit front end (`wccq`,
  `toolchain/owshim`, from `bld/cc/i86`), records its code-generator calls and
  translates them to HIR. A 32-bit front end is a second `wccq` from `bld/cc/386`
  (the tree is cached); it gates the skeleton.
- No `-m` flag and no writer interface: `compile.rs:220-222` calls
  `objbuild::written` or `masm::text`.

## Classes

- **T** target fact: registers, pointer width, legal integers, address spaces,
  calling conventions, costs, forms.
- **O** object-format fact.
- **S** x86 family: true for m16 and m32 (register id, lanes and
  subregisters, bitness are S).
- **C** m16-only: segments, selectors, far calls, `les`, real-mode limits.

Out of scope and pinned to m16: the BASIC runtime, BC raise, Nib (~25k lines).

## Inventory

### Crate graph (the first fact)

| Assumption | Anchor | Class | Becomes |
|---|---|---|---|
| LIR's operand model (`Reg`, `Mem`, `Loc`, `Operation`, `Semantics`, `Effects`) is defined in the BC lifter and typed with `iced_x86::Register` (722 non-test `Register::` in `llrm-core`, 110 in `llrm-bcmachine`) | `llrm-bcmachine/src/model/ir`, `llrm-core/src/model/mod.rs:3` | S | its own crate, `llrm-lir` |
| `llrm-core` depends on `llrm-x86-m16`; the generators pull `parse.rs` in by `#[path]` | `llrm-core/Cargo.toml`, `build.rs`, `generator/mod.rs:17` | T | target crates depend on `llrm-core`, not the reverse |
| `iced_x86` in `llrm-support` (`pyrepr.rs`, `pyset.rs`) and `llrm-omf` | | S | out of support |
| `Profile::target()` builds `Dos`; CPU and target are one axis | `cpu.rs:51` | T | target x CPU (LLVM's subtarget) |

### Registers and classes (T; C where noted)

| Assumption | Anchor | Class | Becomes |
|---|---|---|---|
| Allocatable set is `GENERAL` (six E-registers) | `backend/target.rs:299`, `loopslots.rs:69` | T | `registers().allocatable` |
| Address bases `bx bp si di`; `[bx+si]` roles | `target.rs:19-26`, `regclass.rs:106-135`, `allocate.rs:653,2066`, `select.rs:231-327`, `affine.rs:78` | S (a 16-bit address size, per (mode, address size)) | per-operand register class from the form; the address-form table has the 16-bit and 32-bit forms |
| `width == 2` held value means address or selector | `regclass.rs:111`, `allocate.rs:1810`, `constrain.rs:283` | C | `is_address_value(width)`, selector class empty |
| Selector register file (ES/FS/GS/DS) as a second allocation file | `target.rs:410-475`, `ssaspill.rs`, `regclass.rs`, `constrain.rs`, `splitkit.rs`, `liveness.rs` | C | `selector_registers()`, empty when no segments |
| Fixed registers: mul, div, shifts (cl), string ops, in/out, cwd, restated by hand | `target.rs:67-162` | S | the form table's `fixed` column is the only list; string ops get rows |
| ES/FS pinned for string ops | `target.rs:94,122` | C | `string_op_segments()` |
| Byte-capable registers `ax bx cx dx` | `regclass.rs:73` | S | `byte_registers()` |
| Frame register BP, `Mem.through` | `spiller.rs`, `loopslots.rs`, `floatassign.rs`, `datagroup.rs` | T | `frame_register()`, `stack_pointer()` |
| Callee-saved `si di` | `cpu.rs:69`, `masm.rs:26`, `callregs.rs` | T | `callee_saved()` of the calling convention |
| Lane space: 7 GPR roots + 6 segment + flags | `lanes.rs`, `schedule.rs`, `upperzero.rs`, `peephole.rs` | T | `lane_roots()` |

### Widths and the stack (T)

| Assumption | Anchor | Becomes |
|---|---|---|
| Frame slot and push are 2 bytes | `frame.rs:17`, `spiller.rs:25`, `loopslots.rs:61`, `isel.rs:99`, `parcopy.rs:124` | `stack_slot_bytes()` |
| Return address 2 or 4 bytes; first argument at +4 / +6 | `stackusage.rs:41-71`, `isel.rs:2727`, `peep/guards.rs:301` | `return_address_bytes()`, `first_argument_offset()` |
| `disp_width: 2`, 16-bit frame range | `isel.rs` (11), `frame.rs:155`, `model/lir.rs:17` | `displacement_bytes()`, `frame_reach()` |
| `sp` adjusted as 16-bit | `isel.rs:2957`, `isel/unwind.rs:58`, `prologue.rs:253`, `peephole.rs:177` | `stack_pointer()` + its width |
| iced bitness literal 16 | `select.rs:164`, `declen.rs:15`, `peephole.rs:763`, `regthrash.rs:282`, `inline_asm.rs:449`, `cycles.rs:59` | `bitness()` |
| 16-bit `Code::` names | `select.rs:339-1557` (~40) | `{w}` forms from the instruction table |
| `int(16)` size, index and port types | `hir/mir.rs` (~60), `transforms/algebraic.rs:455`, `calleepop.rs:75`, `window.rs` | `DataLayout` index width, `largest_legal_integer` |
| 66h/67h prefix rules, no single query (mode decides them) | `division.rs:91`, `isel.rs:313-514`, `select.rs:905`, `peephole.rs:1048`, `guards.rs:114`, `cpu.rs:15` | `operand_prefix_bytes(width)`, `address_prefix_bytes(form)`; m32 inverts them |
| Interrupt frame (pushad, segment pushes) | `masm.rs:380-445`, `mir/opcode.rs:357` | calling-convention entry |
| `lower_int64` helper blobs encoded for 16-bit mode | `lower_int64.rs:41-96` | per-target helper table |

### Address spaces and segments (C)

De facto numbering: 0 near/DGROUP, 1 far (selector:offset), 2 selector only,
3 huge, 4 fixed device, 5 near stack (no `p5` entry). Defined in `hir/mir.rs:25-39`,
`mir/types.rs:68`, `mir/datalayout.rs:21`; literals in `analysis/memory.rs`,
`alias.rs`, `consts.rs`, `transforms/inferspace.rs`, `narrowspace.rs`,
`algebraic.rs`, `mir/interpret.rs`.

| Assumption | Anchor | Becomes |
|---|---|---|
| Datalayout string for the program | `hir/mir.rs:29` | target's datalayout (`p:32:32-n8:16:32`) |
| Space numbers as literals | see above (~40) | `address_spaces()`: kind per space {Linear, Pair, SelectorOnly}, `near/stack/fixed` lookups |
| `MemRef.segment/selector`, selector pairs | `analysis/memory.rs:604`, `ranges.rs`, `loopmotion.rs` (~40) | inert when no space is a pair |
| `foreign_span(selectors, offsets)` | `mir/target.rs:13`, `regions.rs:89` | linear range; selector form is a m16 adapter |
| Far loads `les/lds`, far call, `retf`, `farcall.rs`, `farload.rs`, `nearcode.rs`, `combined.rs` | `isel.rs` (~135 selector hits), `peephole.peep:49,209` | gated by `has_segments()` / `has_far_calls()` |
| DGROUP, `datagroup.rs`, `stack_is_data`, `needs_data_group` | `datagroup.rs`, `target.rs:429-507`, `allocate.rs:650-1107` | `segments()` model, `None` when flat |
| Huge pointers: `pointers.rs`, `window.rs`, `huge_shift` | `pointers.rs`, `isel.rs:1586`, `transforms/window.rs` | `huge_window()` is `None` (exists) |
| `Segments`, `es:` overrides, `overriding()` | `select.rs:215`, `masm.rs:700-912` | dropped when no segments |
| HIR `TargetProfile::I386RealMode` only | `hir/model.rs:85`, `verify.rs:235` | profile per target |

### Instruction forms and selection (T / S / C)

| Assumption | Anchor | Class | Becomes |
|---|---|---|---|
| One `x86.instr`, `patterns.isel`, `peephole.peep`; paths hard-coded | `llrm-core/build.rs:9`, `isel/generator/mod.rs:17` | T | per-target definition directory |
| `parse.rs` lives in the m16 crate | `instructions/parse.rs` | S | x86 family crate |
| `jcc/jmp/call` rel16, `call_far`, `les/lds/lfs/lgs` rows | `x86.instr` | C | overlay per target |
| `push/pop` widths, `REGISTERS` list lacks e-registers | `x86.instr`, `parse.rs:63` | T | width-aware names |
| Far, push-pair, `les` peephole rules | `peephole.peep:49,93,209,320` | C | rule sets per target |
| Far patterns | `patterns.isel:8,149,227` | C | pattern sets per target |
| `i64` as two dwords, shifts, x87, `rep movs/stos` shape | `isel/wide.rs`, `select.rs` | S | shared |
| Cost keys name 16-bit forms (`call_far`, `mul_r16`) | `cpu.rs:160-262`, `cycles.rs:207` | T | form-neutral keys |

### What the shared backend assumes of x86

Each is a property of the target's descriptions that passes read, not a default
and not a flag. A target without it has no rows, so its passes find nothing.

| Assumption | Today | Read from |
|---|---|---|
| Register id is `iced_x86::Register` | LIR | opaque id at the allocator interface (PRs 10, 11) |
| Two-address forms | `Machine::two_address`, `twoaddr.rs` | form table ties (`^0`) |
| ALU ops with a memory operand, and the fold and peephole machinery built on them | `rmw.rs`, `storecombine.rs`, `peephole.rs` | form table operand kinds; a load/store ISA has no `m` on ALU rows |
| 8/16-bit subregisters and lanes | `lanes.rs`, `upperzero.rs`, `target.rs:327-392` | register description |
| Fixed registers: string ops, mul, div, shifts | `target.rs:67-162` | form table `fixed` |
| "Memory operands" as a cost metric | peephole and isel pricing | form-table cost keys |
| Segments, far calls, selector holding | see above | address spaces of pair kind; far forms in the table |
| Condition flags as an implicit register | `liveness.rs` | forms' flags read/written columns |

### Object format (O)

| Assumption | Anchor | Becomes |
|---|---|---|
| OMF 16 writer called directly | `compile.rs:220`, `nib/compile.rs:78`, `driver/basic.rs:362` | `ObjectWriter` |
| OMF constants, DGROUP/STACK/`_TEXT` names, USE16 attributes | `llrm-omf/src/write.rs` | inside the OMF writer |
| MASM header `.model medium`, `dd/dw` pointers, `proc far` | `masm.rs:166-217,535` | `Listing` syntax per target |
| CodeView 16-bit records | `codeview.rs`, `cvwrite.rs` | debug writer per format; m32 refuses `-g` first |
| Jump relaxation with rel8/rel16 reach | `objbuild.rs`, `jumps.rs:21` | `branch_forms()` |
| `Space::{Group,Segment,Far}` in the model the backend shares | `datagroup.rs`, `globals.rs`, `masm.rs` | relocation kinds the format interprets |

### Drivers and frontends

| Assumption | Anchor | Becomes |
|---|---|---|
| Open Watcom 16-bit front end, `-mm -zp1 -ecc`, `borland.h` | `compile.rs:127`, `owshim/build.sh:24`, `cgshim.c:130` | per-target front-end profile; 32-bit `wccq` |
| `int`/pointer sizes, `medium_model()` clobbers | `raise_hir.rs:14-122` | target's type widths and ABI |
| `far`, `huge`, `__based`, call distance in `llrm-c` | `hir.rs`, `translate.rs` (~90) | collapse; refuse `__based/__segment/__huge` |
| Flags pick the machine: `-march`, `--machine` | `driver/flags.rs:163-230` | `-m16`/`-m32` select the `Target` by the number its `datalayout.toml` declares |
| QB, BC | `llrm-qb`, `llrm-bc*` | pinned to m16 |
| Nib | `llrm-nib` | m16 and m32: layout, conventions and OS layer come from the target (`runtime/shared/`, `runtime/nib/`) |

## Interface

Names: `Target` (new), `CostModel` (today `llrm_mir::target::Machine`),
`Platform` (today m16's `machine::Machine`, `dos.toml`). Renames land with
the PR that touches each.

```
llrm-mir        MIR, DataLayout, CostModel, address-space kinds    generic
llrm-lir        LIR operand model: Reg, Mem, Loc, Operation,       x86 family for now
                Semantics, Effects (out of llrm-bcmachine)
llrm-core       backend over LIR: allocator, spiller, frame, peephole runtime,
                SelectCx, trait Target, generic call/ret and address-form lowering
llrm-iselgen    shared generators (isel, peephole): run by each target's build.rs
llrm-x86        family: form schema and parser, condition codes, encoder,
                x87 and i64 hooks, string-op shapes
llrm-x86-m16 descriptions, hooks, generated selector, timings
llrm-x86-m32 descriptions, hooks, generated selector, timings
llrm-driver     the one place that names targets: match on -m
llrm-omf        OMF 16 today; 32-bit records later
```

Dependencies point down: target crates depend on `llrm-core`, never the
reverse; the driver depends on both. That is the end state (PR 19b): generated code
lives in the target crate and only calls downward, so there is no cycle. Until
`llrm-core` stops using m16 (PRs 5 to 19), the selectors are generated in
`llrm-core` from each target's definition directory and bound by `llrm-driver`. Tests in `llrm-core` that need m16
move to the target crate.

`Target` gives: `data_layout()`, `address_spaces()`, `registers()`,
`convention(name)`, `cost_model()` (per CPU: target x CPU, LLVM's subtarget),
`forms()`, `object()`. Passes hold `&dyn Target`; hot loops copy what they need
into tables. Capabilities are not a bag of flags: "has segments" is an address
space of pair kind, "has far calls" is far forms in the table.

## MIR to LIR: the GlobalISel pipeline

Adopted from LLVM: GlobalISel's pipeline, not SelectionDAG. Per function, on the
generic machine instruction below, with legality declared in the target's
description.

| Phase | LLVM | llrm |
|---|---|---|
| 1 | IRTranslator | MIR to generic LIR (`G_ADD`, `G_LOAD`, `G_PTR_ADD`, ...), typed with low-level types and address spaces. Target-independent except call lowering, which reads the calling-convention description |
| 2 | Legalizer | the target's legality table per (op, type): legal, widen, narrow, lower, libcall, unsupported. Per (mode, profile) |
| 3 | RegBankSelect | banks from the register description: GPR, x87, segment |
| 4 | InstructionSelect | `patterns.isel`, plus address-mode complex patterns (base + index*scale + disp, segment), declared per target |

### Where today's code sits

There is no generic LIR. `isel::selected()` emits x86 machine LIR directly
(`Semantics` named `mov`, `adc`, `shld`, `idiv`) over virtual registers. It is
not four phases but three stages: a whole-function MIR analysis, a per-instruction
selection in reverse post-order, and four whole-function post-passes. Legalization
is inline: i64 is two dwords in `wides`, a far pointer two words in `fars`, a
dword joined from two words in `joins`, an aggregate call result in `fields`.
RegBankSelect does not exist: banks are implicit (x87 values have width 10; GPR
versus selector is decided late, in `regclass`/`allocate`, from the intersection
of a value's uses). `select.rs` is the encoder after selection, not a selector.

| Region (`backend/`) | Stage | Becomes |
|---|---|---|
| `isel.rs` `fuse`, `narrowed`, `pair`, `covered_by_pattern`, `merged_condition`, `prezeroed`, `phis_from`, `alloca_groups` | MIR analysis before translation (LLVM's CodeGenPrepare and builder decisions): they read MIR users and types | stays on MIR until generic LIR exists |
| `convention`, `passing`, `slot`, `returned` (:65-179), `call`, `called`, `ret` (:2539-2973) | IRTranslator (call lowering) | generic lowering reading the calling-convention description; `Abi` stays for QB's per-callee contracts |
| `body`, phis, `switch`, frame layout with `hole`, `variadic`, interrupt `unsealed`, `lined`, `odds`, `pins`, `inputs` | IRTranslator | generic |
| `width_of`, `size_of` | IRTranslator (type to low-level type) | datalayout |
| `wide.rs` i64, `far_cast`, `float_cast`, `far_loaded`, `memcpy`/`memset` (:2974-3339), `divide`, `compare`, `division.rs`, `arithmetic.rs` | Legalizer | rules; helpers in the x86 family or m16 crate |
| `Pointer`, `indexed`, `widened`, `carried`, `window`, `memory`, `folded` (:1487-2272) | address-mode selection; decided at the GEP from MIR analyses (`facts`, `exact`, `typed`) and edited afterwards by `promote` | for m16 an address-mode analysis on MIR feeding translation; complex patterns on generic LIR are for m32 and arm64 |
| `matcher.rs`, `patterns.isel`, generator | InstructionSelect | stays; gains the hooks' replacements |
| post-passes in order: `unread_halves_dropped`, `combined` (`farload`, `comparefold`, `rmw`, peephole arguments, `dword_pairs`), `widen` (`exact_sums`, `addressforms::promote`, which allocates ids after `next`), `rooted` | after selection | stay x86 family; far parts m16 |
| `farcall.rs` | first machine phase | m16 |
| `selects.rs`, `ehprepare.rs`, `nearcode.rs` | MIR pre-passes | stay MIR |
| `unwind.rs`, `masm`, `frame`, `prologue`, `objbuild` | other | unchanged |
| `lower_int64.rs` | data (helper blobs), name is stale | per-target helper table |
| `pointers.rs` | dead (only its own tests use it) | left as is: nothing is dropped from m16 |

### The legalizer, as `wide.rs` does it

On m16 with a 386+ profile i8, i16 and i32 are legal and i64 narrows to two
dwords, low first. The key of a legality rule is the full type tuple (source and
destination of a cast, shift amount, address space), with type ranges; decisions
made on values (`sign_bits`, a constant count, a power of two) stay inside the
named action. For m16 the table is mostly `custom(helper)`: the action emits
target instructions directly, never generic ops that selection would re-select
(`wide()` puts constant halves in registers; a generic `G_ADD` would select
`add r, imm`). What it does today:

- casts to i64: `movsx`/`movzx` for a narrow source, high is `cdq` (sext) or
  `mov 0` (zext); from i64: the low half, `and 1` for i1; `sitofp` stores the pair
  to an 8-byte temp and `fild`s it; other ops from i64 are unsupported;
- add/sub `add`+`adc`, `sub`+`sbb`; and/or/xor pairwise;
- shifts by a constant only, mod 64: 1..31 `shld`/`shrd` with the half shift, 32
  and over move a half and fill the other; a variable count is unsupported;
- mul: one `imul` when both operands are sign-extended i32s, else `mul` plus two
  `imul`s and adds; compare: constant halves are immediates, `xor`/`xor`/`or` for
  equality, `sub`+`sbb` for order;
- division: signed by a power of two by bias and shifts; signed by a narrow divisor
  by one `idiv` or two `div`s with a sign fix; everything else, all unsigned
  included, by the inline helper blobs (`__I8D`, `__U8D` and the 32-bit-constant
  forms), clobbering AX BX CX DX.

`llvm.smul.fix`/`sdiv.fix` are i32 intrinsics (`imul`+`shrd`, `cdq`/`shld`/`idiv`),
not i64 legalization. The prose is a summary: the oracle below is the spec.

A future pre-386 profile narrows i32 to i16 pairs by the same rule kinds with a
different table.

### The 26 hooks by phase

| Phase | Hook steps | Form |
|---|---|---|
| IRTranslator | `call`, `ret`, `invoke`, `br`, `extractvalue` | generic lowering from the calling-convention description; inside `call`: memset/memcpy are legalizer rules, port in/out and float unary are patterns, ptrdiff/window are m16, va_start is the convention |
| Legalizer | `wide_cast` (2), `wide_binary`, `float_cast` (2), `float_bits`, `far_load`, `far_store`, `far_cast` (2), `unselected_cast`, `divide`, `words` | rules. `far_*` are m16 rules; `float_bits` a generic lowering of a float-constant store; `words` a load-narrowing combine |
| InstructionSelect | `getelementptr`, `scaled`, `setcc` (5) | `getelementptr` per the address-mode row above; `scaled` a pattern in a cost `group` with a chain-generator hook; `setcc` patterns over a condition-code table |
| other | `landing_pad` | runtime hook |

### Staging, byte-identical

0. **A shadow oracle first.** Run the old and the new selector on every function
   of the corpus and compare the isel LIR after the "isel" stage, ids included,
   plus a snapshot of the constant pool. Equal isel LIR and pool mean equal bytes,
   since everything after isel is deterministic. Compare with a dump that prints
   every field, not `==`: `Mem`'s hand-written `Eq` skips `exact`, `through` and
   `disp_width`. It names the function and instruction, which `.obj` comparison
   does not.
1. A legality table in the description. Derive the "today" column statically from
   the generator's opcode x type walk to candidates plus the `refuse(` sites in
   each hook as `unsupported`; a unit test checks the table equals it, and the
   corpus count of `Unselected` messages must not change.
2. Out of `Selector`: the legalizer's state (`wides`, `fars`, `joins`, `fields`,
   `halves`). Three sets stay shared: `paired` (`wide_binary` reads it),
   `consumed` (four analyses write it, patterns read it) and the dead-half
   elimination, which reads `halves` and `folded`.
3. Rules replace the hooks one opcode family per PR, each behind the oracle,
   `wide.rs` first.
4. Generic opcodes for m32 first (below); m16 stays on the fused selector
   behind the same interface and moves over family by family under the oracle.
5. RegBankSelect: for m16 it declares GPR and x87 only and leaves GPR versus
   segment to the allocator (assigning segments early changes code). Its real use
   is m32 and arm64.

Hazards the oracle must cover: value ids are numbered lazily on first reference,
in this order today: parameters in the prologue, `phis_from` in layout order
(before any selection), reverse post-order selection, then `promote`; ids order
iteration in the spiller and coalescer, so one counter is threaded through and the
order kept. The constant pool is module-wide and interns in first-asked order; a
function is selected up to four times (`hole` x spiller/allocator) and rejected
runs still intern. `materialized` and `far_globals` cache per block and are spliced
before the current instruction; `current` is `None` during `phis_from`, so a
far-global phi input is refused today and must stay refused. `scratch` lives for one
MIR instruction; calls and inline helpers are keyed by `at`, and a legalized
result inherits its instruction's `at`. A canonical numbering would itself change
m16's output, so the three stages never run as whole-function sweeps there.

## End state: a generic machine instruction

Decided: LIR becomes a generic machine instruction, designed now and built in
stages. No arm64 code in this task. Today's `Semantics { op, name, dests,
sources }` is close; what is x86-shaped is its operands.

```
MachineInstr
  opcode     OpcodeId: an index into the target's mnemonic list (the forms
             plus raise-only names such as fidiv, wait); the form is found from
             (mnemonic, operand kinds), as the encoder picks the iced Code at emit
             time. Option<OpcodeId>: None is a Barrier
  dests, sources   as today. A tie is the same operand in dests[0] and
             sources[0]; the form declares that it must hold
  operands   Reg(RegId, width)  Imm(value)  Mem(AddressRef, MemInfo)
             Address(AddressRef)  Held(value, width)
  per operand: a register class from the form (LLVM's MCOperandInfo.RegClass)
  implicit   defs, uses, fixed registers and flags stay on `Insn`
             (requires, delivers, clobbers), filled from the form; flags stay a
             bitset, an implicit def or use of one status resource
```

```rust
pub struct RegId(u8);      // ordinal == iced discriminant; NONE = 0; Hash writes an isize as iced's does
pub struct RegisterInfo { name, size, root, lane, classes }   // generated, &'static, held by the pass context
pub struct Slot { value: Option<Held>, reg: RegId }           // reg is today's through/index_through: not in Eq
pub struct AddressRef { base: Slot, index: Slot, scale: u8,
                        segment: Slot,       // today `selector`; only pair-kind spaces declare it
                        disp: i64, disp_bytes: u8 }
pub struct MemInfo { addr: Option<Addr>, width: u32, stack_argument: bool, exact: bool }
pub struct Mem { at: AddressRef, info: MemInfo }              // hand-written Eq/Hash as today
```

- The register id is the target's id from its register description, in iced
  order for m16. Subregisters are ids with a root and a lane (`al` is a view of
  `eax`). x87's stack is a register class with an operand kind the description
  names; `Loc::St` becomes a `Reg` in it.
- The form table declares which `AddressRef` slots are legal; it does not decide
  the layout. A load/store ISA's ALU rows have no `Mem`.
- `Mem` splits into the encoding (`AddressRef`) and the identity alias analysis
  reads (`MemInfo`, LLVM's `MachineMemOperand`): the hand-written `Eq`/`Hash`
  exclusions in `ir/mod.rs` are that split already. `Loc::Address` reuses
  `AddressRef`; its index is a physical register today and a `Held` in `Mem`,
  and they unify.
- `Operation` stays: what an instruction computes, not a mnemonic.
- Register queries (`size`, root, class, is-address-base) take a `&RegisterInfo`;
  no process-wide target. About 55 iced method sites are the whole surface
  (`size` 34, `full_register32` 11, `is_segment_register` 3, ...); build no more.
- BC raise keeps iced in its decoder and converts at `semantics.rs`
  (`_location`, `_register_effects`) with `x86::reg(Register) -> RegId`, the
  identity on the ordinal.
- Debt until `RegId`: LIR names the frame register BP and the stack pointer SP
  whatever the target, and `masm::listing()` spells them as the target's
  `FrameRegisters` has them (EBP, ESP for m32). Object output must go through
  the same `listing()`; a pass that compares `through == BP` is reading a role.
- `llrm_support::register::PhysicalRegister(u32)` exists and is unused: it
  becomes `RegId` or is deleted; never both.

Byte-identical hazards for these PRs, one parity test over `Register::values()`
(iced against `RegisterInfo` on `Ord`, Fx hash, name, size, root, `as usize`):
iced's `Register` hashes an isize, so a `RegId(u8)` derive reorders every
`HashMap`/`HashSet` of registers (`regthrash`, `copysink`, `peephole`,
`spillforward`); `BTreeSet<Register>` (89 sites) and allocation order need
ordinal equal to iced's; dumps print registers by number (`pyrepr.rs`);
`lanes.rs` and `verify.rs` index arrays by `register as usize`; a `derive` on the
new structs silently adds `through`, `exact` and `disp_width` to `Eq`, so keep
the hash field order and pin it with a golden fixture; `name: String` to
`OpcodeId` changes `Semantics`' `Hash`, and its `Repr` must still print the
string; `Register::None` is a sentinel.

Cut as YAGNI: predication, bundles, kill/dead/undef operand flags, an
MCInst/MachineInstr split, subregister-index operands, an explicit `Block`
operand (keep `target` and `indirect`), writeback addressing, register pairs.

Facts a later ISA will need and nothing asks for yet, added when it does: return
address in a register, explicit flag-setting forms, immediate encodability,
16-byte stack alignment and register pairs, PC-relative and GOT relocations,
conditional select.

## PR sequence

Each PR is behaviour neutral and gated on: bench 216 measurements, 0 problems,
no `expected.toml` change; `tools/sizes.py` -O2/-Os equal; QCport's 65 modules
byte-identical (`.obj` compared); workspace lib, `--test run`, `--test check`
green. Each states the `llrm_x86_m16` uses outside the target crate and the
m16-pinned frontends (production / total; 20 / 65 today), and the metric.

| # | PR | Moves |
|---|---|---|
| guard | #502 `test_m16_emits_386_forms` | none |
| 0 | this document and the reviews | none |
| 1 | withdrawn: the MIR crates already take m16 as a dev-dependency (manifests checked) | none |
| 2 | `llrm-lir`: the operand model out of `llrm-bcmachine`, moved unchanged, with `Addr` (via `llrm-omf` for now), `Flag`, `root()` and the `Repr` impls | no m16 or iced in the BC lifter's model crate |
| 3a | `llrm-target`: the platform description (`Machine`, its parser) out of m16, which keeps `dos.toml`, `BUILT_IN`, `BASIC`, `CPUS`; the BC crates stop depending on m16 | production uses 20 to 17 |
| 3b | the platform is flat-capable (`addressing = "flat"`, optional `[segments]`), `dos.toml` parses to an identical value; the PC ports in one shared file | `llrm-target` |
| 3c | `llrm-driver`, `trait Target` (in `llrm-target`: `llrm-core` cannot be below m16 while `llrm-bcmachine` was above it), `-m16`/`-m32` (default m16), the target in `Options`; `llrm-core` stays on m16 for the statics that `allocate`, `regclass`, `constrain` and `ssaspill` read (until PR 19) and for the profile tables (until PR 5); metric baseline | about 12 uses in `llrm-core` left |
| 4 | one selector per target definition directory: `build.rs` generates a `Compiled` for each `crates/target/<name>/src/isel/` (`patterns.isel`, forms in `src/instructions/x86.instr`), found by the directory's name; `llrm-driver` binds a target to its selector and hands it in through `Options`; m16's `patterns.isel` moved there. The generated code and its hooks stay in `llrm-core` until the inversion (after 19) | `build.rs`, `matcher.rs`, `isel.rs`, `assemble.rs` |
| 4b | the same for the peephole rules (`peephole.peep`) | `build.rs`, `peep/` |
| 4c | no default names a target in shared code: `Options`, `assemble`, `flow`, `Peephole`, `Profile` take the selector, rules and model from the driver; tests use a helper | `llrm-core`, `llrm-driver` |
| 5 | `llrm-x86` family crate: schema, parser, condition codes, encoder; byte sizes from the encoder | `parse.rs`, `isel/matcher.rs:366` |
| 6 | the form table is the only list of fixed registers, flags, ties and implicit defs/uses; string-op rows | `target.rs:67-162` (~100 lines) |
| 7 | shadow oracle: old and new selector compared per function (isel LIR dump with every field, pool snapshot) | test tool only |
| 7' | one `TypeClass` that the patterns' type names, `width_of` and `size_of` read; the legality table moves to PR 15 (generic opcodes), where it has a reader | `isel.rs`, `isel/matcher.rs` |
| 9 | the legalizer's state out of `Selector`; three shared sets stay | `isel.rs`, `wide.rs` |
| 10 | register description; `RegId` as an alias of iced's `Register`, `RegisterInfo` queries, generated constants, the parity test | no behaviour change |
| 11 | `RegId` migration in slices by area, one PR each; `regclass`, `allocate`, `ssaspill`, `constrain` get only the mechanical rename after cost-spill; then the alias becomes a newtype | ~830 sites |
| 12 | legalizer rules replace the hooks, one opcode family per PR under the oracle: `wide.rs` first, then `far_*`, then float casts | 13 hooks |
| 13 | calling-convention description and generic call/ret lowering | `isel.rs:65-179`, `callregs.rs` |
| 14a | address-space kinds replace the literals 0/1/2/4/5 in MIR and HIR; `foreign_span` linear | HIR, analysis, transforms |
| 14b | the address-form table per (mode, address size) read by `Pointer`, `indexed`, `select.rs`, `affine.rs` | `isel.rs`, `select.rs` |
| 14c | `CostModel`'s m16 notions behind space kinds | `llrm-mir` |
| 15 | operand model, one PR each (own track, with 10 and 11): `name` to `OpcodeId` (351 sites); `Mem` to `AddressRef` + `MemInfo` (487); `Loc::St` to a `Reg` (47); `Loc::Address` reuses `AddressRef` | `llrm-lir`, every `Semantics` consumer |
| 16 | segment and far passes keyed to pair-kind spaces | `farcall`, `farload`, `nearcode`, `datagroup`, far arms of `isel.rs` |
| 17 | schema edits (four) and language features (i) to (v), one per PR, each deleting its hooks | `isel.rs`, `patterns.isel` |
| 18 | `ObjectWriter` and listing syntax read from the object-format description | `compile.rs`, `basic.rs`, `masm.rs` header |
| 19 | class routing in `regclass`, `allocate`, `ssaspill`, `constrain` | **after cost-spill lands, agreed with it first** |
| 19b | the inversion: generated code and hooks move into the target crates, which then depend on `llrm-core`; nothing in `llrm-core` names m16 by then | `llrm-core/Cargo.toml`, `build.rs`, hooks |
| 20 | owned by the m32 session: `llrm-x86-m32` skeleton, the first client of the generic pipeline (generic opcodes, legalizer table, complex patterns, RegBankSelect): descriptions, 32-bit `wccq`, HIR profile, `-m32`, listing test for `int add(int,int)` and a loop over `int*` | new crate; no shared line changed |

PRs 2 to 4 are the structural ones and go first: every later "where does this go"
depends on them. m32 (20) has no baseline to match, so the generic pipeline is designed there;
7 to 15 move m16 onto the same machinery under the oracle, and the skeleton adds
descriptions, not code. Not in this task: running or linking m32, a 32-bit object
writer, m64, arm64.

The m32 session also makes the shared edits its listing needs, each as a
`Target` query whose m16 answer is today's literal (so byte-identical), one PR
per row: A, `Options` carries the target, with `stack_slot_bytes`, `frame_register`,
`first_argument_offset` and `returns`; B, `callee_saved`; C, the datalayout and HIR
profile into HIR to MIR; D, `bitness` and the listing header and frame in `masm`;
E, the width in `allocate.rs` (cost-spill's file: mechanical, agreed with it first);
and PR 5's data half, the family/m16 split of `x86.instr`, after PR 4b.

PRs 2 to 19 and m16's migration otherwise belong to the target-refactor session. The
m32 session owns PR 20 and what follows: the crate and its descriptions, the
32-bit `wccq`, the 32-bit HIR profile, and the generic-pipeline pieces as its
first client. Its crate stays out of the workspace until PR 3c, it edits no
shared file, and hook requests go to the owner of PRs 2 to 19. It uses `Machine`
from `llrm-target`; a second platform type would be the duplication this plan
removes.

## Metric

Per target, in tokens (rustfmt-proof), comments excluded:

- **R**: Rust in the target crate, plus shared-crate Rust that runs only under
  something this target alone enables.
- **D**: description tokens (not `dos.toml`).
- **G**: tokens in the generators and `SelectCx`. A description language turning
  into a programming language shows up here.

Gate: adding m32 changes no line in a shared crate. Success: R falls by more
than G grows. Hook count and lines deleted from `llrm-core` are secondary
evidence. Baseline, measured in PR 3 before any move; today m16 is 1,724 Rust
lines in its crate and 837 description lines (`x86.instr` 88, `patterns.isel`
380, `peephole.peep` 369), with 26 pattern hooks.

## Open questions

- Whether `llrm-lir` is the family crate or its own: proposed its own, so the BC
  lifter and the backend both depend on it.
- m16's legalizer actions emit target instructions directly (custom). That
  is what byte identity needs under the oracle, and it is debt against "a target
  is description". An action becomes generic ops plus patterns when the patterns
  reproduce its output: costs that keep a constant half in a register where
  `wide()` does, `add r, imm` where it does not, the same carry chains and the
  same register pins. The oracle is the test; a family whose generic form is
  byte-identical moves, the rest stay custom and are listed with the reason.
