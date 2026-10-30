---
architecture-index: Target facts in backend code: audit
architecture-order: 530
---
# Target facts in backend code: audit

Rule: a register, class, encoding, latency, size, alignment, calling convention or cost lives in
the target's description (`llrm_target::Target`, `registers.regs`, `calling.toml`, `opcosts`,
`timings`, `Layout`, `ObjectFormat`) and reaches a pass as a parameter. A pass holds no literal and
no target match.

## What exists

`Target` (`crates/target/llrm-target/src/lib.rs`) already states: spaces, layout, stack slot and
alignment, frame/stack register, callee-saved set, results, address forms, operation costs,
register capacity, float stack, two-address, function alignment, nop size, object format.
`tests/target_facts.rs` bans, outside the target crates: target and CPU names, register names
(quoted or `Register::X`), a long double's size and the segment size (and size − 1). Baseline: 52
files, `tests/target_facts.baseline`. It does not see the classes below, which is why they are
still here.

## Method and limits

Production code of `crates/codegen/llrm-codegen/src` (the code before each `mod tests`), found by
pattern, then read. Counts are hits, not decisions: a hit is a candidate. Tests and
`crates/target` are out of scope. Hit totals: mnemonic 546, size 119, register 93, mode 28,
iced 10, cost 6.

## A. LIR vocabulary is x86 (546 hits): the structural one

Passes name x86 mnemonics as strings (`"mov"`, `"push"`, `"lea"`, `"idiv"`, `"cwd"`, `"rep"`…).
Largest: `isel.rs` 130, `isel/wide.rs` 82, `peephole.rs` 50, `division.rs` 29, `masm.rs` 25,
`isel/matcher.rs` 25, `spiller.rs` 18, `arithmetic.rs` 17, `pointers.rs` 15, `schedule.rs` 11,
`rmw.rs` 10, `affine.rs` 9, `parcopy.rs` 7, `upperzero.rs` 6; ~40 more files with 1–3.
Belongs in: a generic LIR opcode set (move, push, pop, load-address, add, …) with the target
mapping it to its encodings; passes ask `Target` for the generic opcode (`target.ops().mov`) and
match on it. This is the arm64 spike's `RegId`/`Flag`/opcode change: do not start it here.

## B. Mode (16/32) tests (28 hits)

The `m16()`/`M16` defaults (`assemble.rs`, `cpu.rs`, `driver/mod.rs`, `flow.rs`, `classes.rs`, `registerinfo.rs`, `floatalloc.rs`) are `#[cfg(test)]` helpers: not facts in a pass. `Peephole::new` and `floatalloc::allocated` are test-only too. None in production.

| file:line | fact | belongs in |
|---|---|---|
| `spiller.rs:1626` | `body.bits == 32` selects the slot model | `Target::stack_slot_bytes`, `Layout` |
| `objbuild.rs:618,716,730,769,800,825,1124,1180` | `bits == 16/32`: nop style, displacement width, far callee, offset fixup kind, near/far, `Arch::I386/I8086` | `ObjectFormat` + `Layout` (relocation kinds, machine id, near-call width) |
| `masm.rs:721`, `framefree.rs:159`, `debuginfo.rs:671,712` | `bits == 32` | `Layout`, `FrameRegisters` |
| `cfi.rs:101` | 16-bit register spelling drops `e` | `registers.regs` name per width |
| `classes.rs:125`, `inline_asm.rs:114` | `one.bits == 16`, `bits == 32` | `registers.regs` |
| `isel.rs:3353` | `bits >= 32` | `Layout` |

## C. Registers by name (93 hits; the ratchet sees only strings and `Register::`)

| file:line | fact | belongs in |
|---|---|---|
| `inline_asm.rs:46–72`, `classes.rs:255` | name↔register tables (`"ax"`, `Part::Low`) | `registers.regs` |
| `lower_int64.rs:67–75` | `EAX→Ax`, flags | `registers.regs` aliases |
| `abi/qb.rs:197–204,361` | runtime register set (`Ax…Es`), `Bx` input | `runtime.toml` / `calling.toml` |
| `allocate.rs:1863` | `RFP = MM0..MM6` float-register order | `registers.regs` class order |
| `callregs.rs:134`, `ssaspill.rs:1288`, `machinecp.rs:391`, `masm.rs:847`, `frame.rs:416`, `verify.rs:96` | `ES`, `DS`, `CX`, segment list | `registers.regs` roles (segment class, count register) |
| `isel.rs:3251`, `loopslots.rs:567`, `spiller.rs:3396`, `peephole.rs:267` | `BP`/`SP`/`EBP`/`ESP` as the frame base | `Target::frame_register`, `stack_pointer` |
| `floatassign.rs:343`, `driver/basic.rs:184` | `RegId::None/…`, `EAX` as result | `Target::results` |

## D. Sizes, widths, alignment (119 hits)

| file:line | fact | belongs in |
|---|---|---|
| `isel.rs:2520,5808,5836`, `spiller.rs:1739`, `allocate.rs:5689–5699` | slot/push step 2 or 4 | `Target::stack_slot_bytes` |
| `abi/qb.rs:414` (24 hits) | argument widths and byte counts of runtime routines | `calling.toml` |
| `pointers.rs:93`, `pointers.rs` header | `& 0xffff`, `>> 16`, page size | `Layout` segment fields (the ratchet catches the segment size only in code it finds; these are masks) |
| `objbuild.rs` OFFSET/OFFSET32 | offset width | `ObjectFormat` |
| `masm.rs:1868` | `push`/`pushd` by width | target encoding table |
| `isel/matcher.rs:896`, `schedule.rs:220`, `rmw.rs:328`, `farload.rs:128`, `storecombine.rs:79`, `regclass.rs:339`, `framefree.rs:233`, `addressforms.rs:94`, `constrain.rs:402`, `floatassign.rs:174`, `peep/guards.rs:250`, `isel/combined.rs:197`, `arithmetic.rs:224`, `cpu.rs:258` | `width == 2` / `== 4` meaning "native word" or "address register" | `Layout::pointer`/`word`, `AddressForm` |
| `constpool.rs:158`, `timing.rs:71` | 8-byte constant test; `smul_full_w2` | `Layout`; `timings` |
| `division.rs:108` | magic-number word size | `Layout::word_bits` |

## E. Costs and latencies (6 hits, but the cost model is the clean one)

`schedule.rs`, `cpu.rs` and `timing.rs` read `Cpu`/`Profile` from the description: correct. Left:
`jumps/placement.rs:29` `TAIL_DUPLICATION_PENALTY = 0.02` (LLVM's is a cl::opt, not a target
fact: keep, cite). `parcopy.rs`, `frequency.rs`, `shrinkwrap.rs` "cycle" hits are graph
cycles, not machine facts.

## F. iced-x86 types in generic crates

The x86 decoder's types are the register and flag vocabulary of the whole tree, not only of the
passes. Generic crates hold none.

| crate / file | iced types | belongs in |
|---|---|---|
| `llrm-lir/src/registers.rs`, `flag.rs`, `build.rs` | `Register` (the `RegId` payload), `RflagsBits` (the `Flag` payload) | per-target `RegId`/`Flag` (arm64-spike's generification) |
| `llrm-target/src/registers.rs` | `Register` as the id type | same |
| `llrm-support/src/{pyset,pyrepr,lib}.rs` | `Register` (hash, repr), `Code`, `Decoder` | the target crates; support keeps no x86 |
| `llrm-codegen`: `peephole.rs` (5), `postrasink.rs` (2), `verify.rs`, `valuetrack.rs`, `target.rs`, `regthrash.rs`, `registerinfo.rs`, `peep/guards.rs`, `cfi.rs`, `arrival.rs` | `Register`, `RflagsBits`, `Decoder`, `Instruction`, `InstructionInfoFactory`, `Code`, `FlowControl`, `Mnemonic`, `OpKind`, `OpAccess` | a target `Encoding` interface: flag effects, flow kind, operand access, decode |
| `llrm-peepgen/src/table.rs`, `llrm-iselgen/src/build.rs` | `Code`/`Register` tables | generator input read from the target description |

One inventory with arm64-spike: this section is the list its `RegId`/`Flag` change closes.

## Ratchet gaps (the fail-first for each move)

`target_facts.rs` should also ban, in shared code: `RegId::<NAME>` (names from the description),
`.bits == <16|32>`, `RegisterClasses::m16()`/`m32()`, `llrm_x86_*::` paths, `width == 2|4`
beside an address/pointer word, `0xffff`/`0x1_0000` masks, `iced_x86`. Each baseline gets the
current count per file and only shrinks.
