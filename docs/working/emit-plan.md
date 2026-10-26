# Emit compile-time bucket: dated plan to the pin's AsmPrinter and MC

Owner w-8. Ledger command (docs/working/ledger-command.md): qbq-llrm render/d_faces.c, `-m32 -mabi=sysv -march=i486 -O2`, callgrind, dist of 0ed9c77f6b. Bucket: 15.5 M ours, 4.1 M pin (`X86AsmPrinter::runOnMachineFunction` + `AsmPrinter::doFinalization`), 3.77x, +11.4 M. QCport 13.1 M vs 3.8 M (ledger `ledger-qcport-0ed9c77f6b-O2.txt`; profiled the same way below).

## Attribution

Taken with `valgrind --tool=callgrind --toggle-collect=llrm_codegen::backend::objbuild::written_in` on the ledger command: 15,483,373 Ir, equal to the ledger's inclusive `written_in`, so the toggle collects the bucket and nothing else. Self cost in the bucket, then inclusive of the pieces:

| Where | M Ir | Calls | What it is |
| --- | --- | --- | --- |
| `masm::listing` > `frame_omitted` (inclusive) | 6.41 | 7 | frame-register elimination done while printing: `stack_addressed` walks each procedure, clones its instructions, and formats strings |
| of which `Semantics::clone` + `memcpy` + `drop` | 3.0 | 11,055 clones | 9,134 clones in `frame_omitted`, 1,921 in `masm::built`; ~6 per emitted instruction |
| of which `frame_omitted` self | 2.46 | | the walk |
| `objbuild::_items` > `select::emit_in` (inclusive) | 4.44 / 3.64 | 2,252 / 1,714 | the per-instruction encoder, 2.1 K Ir per instruction |
| of which `format!` / `fmt::write` / `pad_integral` | 1.8 | ~9,750 writes | a string built per operand in `select::*` (50 `format!` sites) |
| of which iced `encode` + `decode` | 0.7 | 2,504 / 2,272 | `_assemble` encodes, then decodes the bytes to find the displacement and immediate offsets |
| `masm::spelled` + `Vec<Loc>` collect | 0.72 | 2,252 | operands rebuilt for every item |
| `llrm_omf::write::write` (inclusive) | 1.32 | 1 | 0.60 M of it `format!` (484 record names); OMF has no counterpart in the pin's ELF/COFF path |
| `IndexMap<String,i64>` insert/get, `BodyRoots` fact | 0.64 | | name table and a per-procedure fact |
| `_relaxed` | 0.46 | 1 | jump relaxation |

QCport render/d_faces.c, same command and toggle: 13,061,556 Ir, the same shape. Self cost: `frame_omitted` 2.11 M (16%), `Semantics::clone` 1.21 M + `memcpy` 0.86 M + drop 0.34 M (2.4 M), `_items` 0.51 M, `Vec<Loc>` collect 0.47 M, `_assemble` 0.43 M, `format!`/`write_str`/`fmt::write`/`pad_integral` 1.2 M, `omf::write` 0.35 M, `BodyRoots` fact 0.25 M, `_relaxed` 0.22 M. Each row's PR states both figures.

What the pin does and ours does not: lowers each `MachineInstr` to an `MCInst` once (`X86MCInstLower`), encodes it table-driven with its fixup offsets recorded as it goes (`X86MCCodeEmitter`), and has the frame already eliminated (PrologEpilogInserter) before the printer runs. Ours eliminates the frame register at emit, copies each instruction several times, builds a string per operand, and decodes what it just encoded. At `-Os` `frame_omitted` also encodes the whole procedure twice to compare sizes, which this `-O2` ledger does not show.

## Rows

One translated change each, in the order they land. A row is measured with the command above and the QCport ledger, and its PR states both.

| # | Change | Pin mirror | Removes (M Ir, est.) | PR date |
| --- | --- | --- | --- | --- |
| EM-1 | The frame register is eliminated in the machine phase (`framefree.rs` already decides it before allocation): the cell offsets are final in LIR and `listing` reads them; `stack_addressed` leaves the emitter | `PEI::replaceFrameIndices`, `X86RegisterInfo::eliminateFrameIndex` | 4.0 | 11-06 |
| EM-2 | `listing` and `built` borrow the procedure's `Semantics` (`Cow`) and no item is cloned per trial | `X86MCInstLower::Lower` once per instruction | 2.5 | 11-06 |
| EM-3 | The `select::*` encoders take enum keys and operands, no `format!` per operand | tablegen'd opcode enum, `MCInst` operands | 1.5 | 11-10 |
| EM-4 | `_assemble` reads the displacement and immediate offsets from the encoder (`Encoder::get_constant_offsets`), no decode | `MCCodeEmitter` records fixups while encoding | 0.7 | 11-10 |
| EM-5 | `spelled` and the name table: operands are built once, names keyed by id | `X86MCInstLower` operand lowering | 1.0 | 11-13 |
| EM-6 | The OMF writer writes record bytes directly, no `format!` per record name | `MCObjectWriter` (ELF/COFF; OMF has none) | 0.8 | 11-13 |

Sum of the estimates 10.5 M of the 11.4 M; the remainder is `_relaxed` and what the rows leave. EM-1 and EM-2 overlap (the clones are inside `frame_omitted`): the second is measured after the first lands. The QCport profile is taken with the same `--toggle-collect` after every row.

## Doubts

- The pin's 4.1 M is the ledger's, not re-measured here.
- Estimates are the attribution above, not predictions: each row's PR carries the measured number.
- `frame_omitted` is called for every procedure at `-O2`, though its trial encoding runs only under `-Os`.
