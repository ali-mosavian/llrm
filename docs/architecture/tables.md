# Tables and generated code

Survey of the rich route (2026-09-27, at `af576024`): which code states facts
that belong in a description, from which the code could be read or generated.
The old `--legacy` route is left out; it is to be deleted, not converted.

About 11k lines of production code qualify, 13k with the Nib parser, plus
8.2k lines of generated tests. Seven descriptions cover them, each replacing
facts now written in several places.

| # | Description | Replaces | LOC | Risk |
|---|---|---|---|---|
| 1 | Runtime (`llrm-qbruntime/src/runtime.toml`) | qbruntime `VARIANTS`, `WRITERS`, `ENTERS_USER_CODE`, `ERROR_FUNNEL_INPUTS`, `FLAGS_RESULTS`; the ~40 `B$X`/pushed-byte refinements in `llrm-core/src/abi/qb.rs`; qbfront `Statement::Runtime` and by-type routine choice (`B$RDI2`..., `B$STI2`..., `FCVx`/`MCVx`); llrm-qb's `B$*USED` and implied `B$ENRA`/`B$EXSA`/`B$CENP` | 2,400 | Low; the format exists. Needs a rule vocabulary for pushed-byte counts and families. |
| 2 | Instruction (TableGen-like) | `select.rs` emitters, `masm::_instruction`, `target.rs` requirements, `schedule::_form`, cost keys in `floatassign`/`allocate`; register tables in `target.rs`/`select.rs`; condition codes and flags in `peephole`, `isel`, `floatalloc`, `layout` (flags could come from iced); call conventions in `isel`; isel's opcode arms; cost tables in `cpu.rs`, `timings.rs`, `passes.rs`; LIR's `mir::Kind` descriptor (or delete it) | 2,750 | Medium; `select_sweep` pins every encoding. |
| 3 | Peephole rewrite rules | Window passes in `peephole.rs` and `isel/combined.rs` | 1,000 | High; guards and pass order dominate. |
| 4 | IR opcodes (like LLVM's `.def`) | HIR `Op` classes and lowering (`model.rs`, `verify.rs`, `mir.rs`); MIR binary/cast properties and class bits; simple parse/print templates; the commutative, pure and may-trap lists copied into transforms; one evaluator shared by `consts.rs` and `interpret.rs`; interval transfer in `ranges.rs`/`floatbounds.rs`; affine decomposition in `induction.rs`, `ranges.rs`, `algebraic.rs`, `indvars.rs` | 1,000 | Low to medium; the copied lists differ today, so unifying them changes behaviour. Diff the suite. |
| 5 | Algebraic rules (like GCC's `match.pd`) | `algebraic.rs`, `canonical.rs`, and their copy in llrm-mir's `instcombine.rs` | 470 | Medium; first match wins, one rule needs an analysis hook. |
| 6 | Formats | HIR codec records (`codec.rs`); OMF and CodeView records (`omf.rs`, `cvinfo.rs`); Watcom code-generator vocabulary and stream schema (llrm-c); BASIC object layout, now in `driver/basic.rs`, llrm-qb and llrm-bcdriver | 2,400 | Medium; byte-exact round trips, FIXUPP threads carry state. |
| 7 | Grammars | Hand-written half of qbfront's `generated_parser/ast.rs`; optionally the Nib parser | 900 (+1,800) | High for Nib: layout-sensitive, context lookahead. |

Smaller: the pipeline, `-O` level and machine phase lists as one table (200);
float enums through `str_enum!` (100); `select_sweep.rs` as a data file with
a small driver (8,200 test lines).

## Found on the way

- No non-test users: qbruntime's `_print` and `_read` (removed by PR #57).
- The commutative set is written out seven times and the pure set in several
  forms, and they already disagree. Description 4 exists mainly to end that.
- `consts.rs`'s `ARITH` shift entries are unreachable and hard-wired to 32 bits.

## Order

1. Runtime: the most lines at the lowest risk.
2. IR opcodes: ends the drifting lists.
3. Instruction: the largest, made safe by `select_sweep`.
4. HIR codec, then OMF.

Peephole rules and the Nib grammar wait until these land.

## Done

- 3, peephole rules: `backend/peephole.peep`, compiled by `llrm-peepgen` into
  one decision automaton per group over `x86.instr`. `constants` (a
  register-contents dataflow) and `tested` (moves an instruction across a
  block edge) are not window rewrites and stay code, as do the passes the
  task left out. `isel/combined.rs`'s push selections
  are rules too, matching a held value's definition; `dword_pairs` and
  `_rematerialized_arguments` stay code.
