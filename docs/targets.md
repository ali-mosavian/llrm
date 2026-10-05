# Targets

llrm builds code for one target today, x86 real mode (`x86-code16`). This
document is the plan to make a target a crate plus descriptions: the inventory of
what the shared code assumes, the interface that replaces the assumptions, what
each crate owns, and the PR sequence. Optimization targets are in
[measurement/targets.md](measurement/targets.md).

Targets the design must fit: `x86-code16`, `x86-code32` (flat), `x86-code64`,
`arm64`. Only code32 is built in this task.

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
| timings | line table per CPU | clocks and prefix costs; bytes come from the encoder |
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

### The 26 selection hooks (19 distinct)

| Hook (steps) | Class | How |
|---|---|---|
| `getelementptr` (1) | generic | reads the address-form table; the far fold is a code16 hook |
| `words` (1) | generic | load narrowing before selection |
| `scaled` (1) | x86 family hook | shift/add chain search priced from timings |
| `divide` (1) | pattern | `cwd` or zero, then `div`, with `partner` |
| `wide_binary`, `wide_cast` (3) | pattern, x86 family | half constructors; shift by count and i64 div stay hooks |
| `float_bits` (1) | generic MIR | a store of a float constant becomes a store of the same bits |
| `float_cast` (2) | pattern, x86 | `temp(bytes)` and alias |
| `extractvalue` (1) | generic | call-result fields come from the convention |
| `setcc` (5), `br` (1) | pattern | condition-code table; the `cover` steps exist |
| `ret` (1) | generic | convention; far and joined-word results are code16 hooks |
| `call` (1) | split | intrinsics are patterns; memset/memcpy are x86 hooks; ptrdiff/window are code16; va_start is the convention; real calls are generic lowering |
| `invoke` (1) | generic | call plus a jump to the normal successor |
| `landing_pad` (1) | runtime hook | helper call that clobbers everything |
| `far_load`, `far_store`, `far_cast` (4) | code16 hook | pair pointers only |
| `unselected_cast` (1) | pattern | `refuse mnemonic` |

About half the steps are generic lowering or loops that stay Rust; they move
behind the calling-convention and address-form descriptions, not into patterns.

## Target facts, in one line each

- **code16**: real mode. Segments, selectors, far calls, 16-bit addressing
  (`bx/bp/si/di`), dword ops under a 66h prefix.
- **code32**: CS = DS = SS, flat, base 0, 4 GB. One pointer type, `p:32:32`. No
  selectors. ES = DS is a fact the description states, not an assumption: string
  ops read it or set ES. FS and GS are never assumed (TLS). Any GPR is a base,
  any but ESP an index. Word ops pay 66h.

## Measured state

- `llrm_x86_code16` is named on 65 lines outside its crate: 20 in production
  code, 45 in tests. The MIR crates use it in tests only but depend on it as a
  normal dependency.
- `llrm_mir::target::Machine` and `DataLayout` are the seam MIR passes read.
  Those passes name no register; what remains in them is address-space numbers
  and 16-bit literals. `Machine` itself carries code16 notions
  (`far_access_registers`, `segment_registers`, `foreign_span(selectors, ..)`,
  `huge_window`, `OperationCosts.copy` as `rep movs`/ES, `carry`).
- `llrm-c` does not use clang. It runs Open Watcom's 16-bit front end (`wccq`,
  `toolchain/owshim`, from `bld/cc/i86`), records its code-generator calls and
  translates them to HIR. A 32-bit front end is a second `wccq` from `bld/cc/386`
  (the tree is cached); it gates the skeleton.
- No `--target` flag and no writer interface: `compile.rs:220-222` calls
  `omfwrite::written` or `masm::text`.

## Classes

- **T** target fact: registers, pointer width, legal integers, address spaces,
  calling conventions, costs, forms.
- **O** object-format fact.
- **S** x86 family: true for code16 and code32 (register id, lanes and
  subregisters, bitness are S).
- **C** code16-only: segments, selectors, far calls, `les`, real-mode limits.

Out of scope and pinned to code16: the BASIC runtime, BC raise, Nib (~25k lines).

## Inventory

### Crate graph (the first fact)

| Assumption | Anchor | Class | Becomes |
|---|---|---|---|
| LIR's operand model (`Reg`, `Mem`, `Loc`, `Operation`, `Semantics`, `Effects`) is defined in the BC lifter and typed with `iced_x86::Register` (722 non-test `Register::` in `llrm-core`, 110 in `llrm-bcmachine`) | `llrm-bcmachine/src/model/ir`, `llrm-core/src/model/mod.rs:3` | S | its own crate, `llrm-lir` |
| `llrm-core` depends on `llrm-x86-code16`; the generators pull `parse.rs` in by `#[path]` | `llrm-core/Cargo.toml`, `build.rs`, `generator/mod.rs:17` | T | target crates depend on `llrm-core`, not the reverse |
| `iced_x86` in `llrm-support` (`pyrepr.rs`, `pyset.rs`) and `llrm-omf` | | S | out of support |
| `Profile::target()` builds `Dos`; CPU and target are one axis | `cpu.rs:51` | T | target x CPU (LLVM's subtarget) |

### Registers and classes (T; C where noted)

| Assumption | Anchor | Class | Becomes |
|---|---|---|---|
| Allocatable set is `GENERAL` (six E-registers) | `backend/target.rs:299`, `loopslots.rs:69` | T | `registers().allocatable` |
| Address bases `bx bp si di`; `[bx+si]` roles | `target.rs:19-26`, `regclass.rs:106-135`, `allocate.rs:653,2066`, `select.rs:231-327`, `affine.rs:78` | C | `address_class(role, width)`; code32: any GPR, ESP not an index, scale 1/2/4/8 |
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
| 66h/67h prefix rules, no single query | `division.rs:91`, `isel.rs:313-514`, `select.rs:905`, `peephole.rs:1048`, `guards.rs:114`, `cpu.rs:15` | `operand_prefix_bytes(width)`, `address_prefix_bytes(form)`; code32 inverts them |
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
| `foreign_span(selectors, offsets)` | `mir/target.rs:13`, `regions.rs:89` | linear range; selector form is a code16 adapter |
| Far loads `les/lds`, far call, `retf`, `farcall.rs`, `farload.rs`, `nearcode.rs`, `combined.rs` | `isel.rs` (~135 selector hits), `peephole.peep:49,209` | gated by `has_segments()` / `has_far_calls()` |
| DGROUP, `datagroup.rs`, `stack_is_data`, `needs_data_group` | `datagroup.rs`, `target.rs:429-507`, `allocate.rs:650-1107` | `segments()` model, `None` when flat |
| Huge pointers: `pointers.rs`, `window.rs`, `huge_shift` | `pointers.rs`, `isel.rs:1586`, `transforms/window.rs` | `huge_window()` is `None` (exists) |
| `Segments`, `es:` overrides, `overriding()` | `select.rs:215`, `masm.rs:700-912` | dropped when no segments |
| HIR `TargetProfile::I386RealMode` only | `hir/model.rs:85`, `verify.rs:235` | profile per target |

### Instruction forms and selection (T / S / C)

| Assumption | Anchor | Class | Becomes |
|---|---|---|---|
| One `x86.instr`, `patterns.isel`, `peephole.peep`; paths hard-coded | `llrm-core/build.rs:9`, `isel/generator/mod.rs:17` | T | per-target definition directory |
| `parse.rs` lives in the code16 crate | `instructions/parse.rs` | S | x86 family crate |
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
| Register id is `iced_x86::Register` | LIR | opaque id at the allocator interface (PR 14) |
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
| OMF constants, DGROUP/STACK/`_TEXT` names, USE16 attributes | `omfwrite.rs:27-62,462` | inside the OMF writer |
| MASM header `.model medium`, `dd/dw` pointers, `proc far` | `masm.rs:166-217,535` | `Listing` syntax per target |
| CodeView 16-bit records | `codeview.rs`, `cvwrite.rs` | debug writer per format; code32 refuses `-g` first |
| Jump relaxation with rel8/rel16 reach | `omfwrite.rs:673`, `jumps.rs:21` | `branch_forms()` |
| `Space::{Group,Segment,Far}` in the model the backend shares | `datagroup.rs`, `globals.rs`, `masm.rs` | relocation kinds the format interprets |

### Drivers and frontends

| Assumption | Anchor | Becomes |
|---|---|---|
| Open Watcom 16-bit front end, `-mm -zp1 -ecc`, `borland.h` | `compile.rs:127`, `owshim/build.sh:24`, `cgshim.c:130` | per-target front-end profile; 32-bit `wccq` |
| `int`/pointer sizes, `medium_model()` clobbers | `raise_hir.rs:14-122` | target's type widths and ABI |
| `far`, `huge`, `__based`, call distance in `llrm-c` | `hir.rs`, `translate.rs` (~90) | collapse; refuse `__based/__segment/__huge` |
| Flags pick the machine: `--cpu`, `--machine` | `driver/flags.rs:163-230` | `--target` selects the `Target` |
| QB, Nib, BC | `llrm-qb`, `llrm-nib`, `llrm-bc*` | pinned to code16 |

## Interface

Names: `Target` (new), `CostModel` (today `llrm_mir::target::Machine`),
`Platform` (today code16's `machine::Machine`, `dos.toml`). Renames land with
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
llrm-x86-code16 descriptions, hooks, generated selector, timings
llrm-x86-code32 descriptions, hooks, generated selector, timings
llrm-driver     the one place that names targets: match on --target
llrm-omf        OMF 16 today; 32-bit records later
```

Dependencies point down: target crates depend on `llrm-core`, never the
reverse; the driver depends on both. Generated code lives in the target crate and
only calls downward, so there is no cycle. Tests in `llrm-core` that need code16
move to the target crate.

`Target` gives: `data_layout()`, `address_spaces()`, `registers()`,
`convention(name)`, `cost_model()` (per CPU: target x CPU, LLVM's subtarget),
`forms()`, `object()`. Passes hold `&dyn Target`; hot loops copy what they need
into tables. Capabilities are not a bag of flags: "has segments" is an address
space of pair kind, "has far calls" is far forms in the table.

Where generic ends. MIR, `Target`, address spaces, `CallingConv`, `ObjectWriter`
and allocation over an id, classes and lane masks are generic. The LIR,
selector runtime, peephole, two-address folding, memory-operand folding,
`rmw`, `storecombine`, `masm` and lanes are x86 family today. **Decision for
the coordinator:** arm64 either gets its own lowering below the allocator
interface, or LIR becomes a generic machine instruction (opcode id, operands,
ties, implicits). This task does not decide it and claims no more than the
x86 family; do not leave LIR looking generic.

Register id: an opaque id (`llrm_support::PhysicalRegister`) at the allocator
interface, a typed family register in the x86 crates. `BTreeSet<Register>` order
follows iced's discriminants, so a renumbered id changes allocation and spill
order; the id keeps iced order, with a test.

Facts a later ISA will need and nothing asks for yet, added when it does: return
address in a register, explicit flag-setting forms, immediate encodability,
16-byte stack alignment and register pairs, PC-relative and GOT relocations,
conditional select.

## PR sequence

Each PR is behaviour neutral and gated on: bench 216 measurements, 0 problems,
no `expected.toml` change; `tools/sizes.py` -O2/-Os equal; QCport's 65 modules
byte-identical (`.obj` compared); workspace lib, `--test run`, `--test check`
green. Each states the `llrm_x86_code16` uses outside the target crate and the
code16-pinned frontends (production / total; 20 / 65 today), and the metric.

| # | PR | Moves |
|---|---|---|
| 0 | this document and the reviews | none |
| 1 | MIR crates take code16 as a dev-dependency | manifests |
| 2 | `llrm-lir`: the operand model out of `llrm-bcmachine` | no code16 or iced in the BC lifter's model |
| 3 | `llrm-driver` and `trait Target`; `--target` flag, default code16; `Profile::target()` builds through it; `llrm-core` reads `GENERAL`, `PRESERVED`, `FRAME`, bases, indexes, bitness from it | production uses to ~0 in `llrm-core` |
| 4 | generated code per target: `llrm-iselgen`, hooks resolved at generation, `CONSTRUCTORS` into the `.isel` header, generators run from the target's `build.rs` | `build.rs`, `matcher.rs`, `peephole.rs` |
| 5 | `llrm-x86` family crate: schema, parser, condition codes, encoder; byte sizes come from the encoder | `parse.rs`, `isel/matcher.rs:366` |
| 6 | the form table is the only list of fixed registers and flags; string-op rows; `requirements()` reads it | `target.rs:67-162` (~100 lines) |
| 7 | calling-convention description and generic call/ret lowering | `isel.rs:68-180`, `callregs.rs` |
| 8 | register description: classes, kind letters, byte registers, lanes | `target.rs` tables, `regclass` reads after cost-spill |
| 9 | datalayout and address spaces: string and space map out of `hir/mir.rs`; literals 0/1/2/4/5 become queries; the address-form table; `foreign_span` linear; `Machine`'s code16 notions behind space kinds | HIR, analysis, transforms, `select.rs`, `affine.rs` |
| 10 | segment and far passes keyed to pair-kind spaces | `farcall`, `farload`, `nearcode`, `datagroup`, `pointers`, far arms of `isel.rs` |
| 11 | schema edits (four) and language features (i) to (v), one per PR, each deleting its hooks | `isel.rs`, `patterns.isel` |
| 12 | `ObjectWriter` and listing syntax read from the object-format description | `compile.rs`, `basic.rs`, `masm.rs` header |
| 13 | class routing in `regclass`, `allocate`, `ssaspill`, `constrain` | **after cost-spill lands, agreed with it first** |
| 14 | register id: family newtype with the same `Ord`, then the opaque id at the allocator interface | ~850 sites |
| 15 | `llrm-x86-code32` skeleton: descriptions, 32-bit `wccq`, HIR profile, `--target x86-code32`, listing test for `int add(int,int)` and a loop over `int*` | new crate; no shared line changed |

PRs 2 to 4 are the structural ones and go first: every later "where does this go"
depends on them. Not in this task: running or linking code32, a 32-bit object
writer, code64, arm64.

## Metric

Per target, in tokens (rustfmt-proof), comments excluded:

- **R**: Rust in the target crate, plus shared-crate Rust that runs only under
  something this target alone enables.
- **D**: description tokens (not `dos.toml`).
- **G**: tokens in the generators and `SelectCx`. A description language turning
  into a programming language shows up here.

Gate: adding code32 changes no line in a shared crate. Success: R falls by more
than G grows. Hook count and lines deleted from `llrm-core` are secondary
evidence. Baseline, measured in PR 3 before any move; today code16 is 1,724 Rust
lines in its crate and 837 description lines (`x86.instr` 88, `patterns.isel`
380, `peephole.peep` 369), with 26 pattern hooks.

## Open questions

- LIR generic or x86 family (Interface, Decision).
- Whether `llrm-lir` is the family crate or its own: proposed its own, so the BC
  lifter and the backend both depend on it.
