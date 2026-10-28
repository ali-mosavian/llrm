# QB front (BC-checked parser) handoff

Branch: `spike/lalr-nib-qb-rebased` = 61 commits on top of `origin/feat/parity` (2dd8f2130 base moved to the
parity tip at the time of the rebase). Same content as `origin/spike/lalr-nib-qb` (force-pushed once with
`--force-with-lease` after the rebase; that ref and this one differ only by this note).

## Done

- `qbfront::parse` = `front::ast::parse` + `front::check::check`, for QBasic, QB 4.5, PDS 7.1, VBDOS and
  QuickrBASIC. Real BC.EXE under the dosrun DOSBox-X fork is the oracle (`crates/frontends/qb/lalrspike`:
  `programs.py`, `more.py`, `bc_programs.json` = 12,898 programs; `bcq.sh`/`verdict`/`checkfiles` tools).
- Readable diagnostics: `ParseError { span, code, message, help, expected, subject }`, wording in
  `front/messages.txt`, `explain.rs`; `driver::render` prints file:line:col, the source line with a caret, help.
- The old table-driven parser, `build.rs`, `crates/buildprs`, `grammar/` and the generated tables are deleted.
  Tokens are `TokenKind::Reserved(Kw)`; the Quickr rewrite passes live in `front/quickr.rs`.
- Lexing: `front/scan.rs` (hand written, uses the logos `Word` DFA for reserved words) is what the parser
  uses; `front/lexer.rs` is a leftover logos path (only `logical_lines` is still called) and can go.
- HIR of all fixture programs (vbdos, pds71, qb45, quickr) was byte-identical before and after the old-parser
  removal. `llrm-qb` tests pass except the red tests below.
- Speed (`cargo run --release --example speed -p qbfront -- vbdos FILES`): on 330 programs / 235 KB:
  lex 41 MB/s, lex+parse 13 MB/s, +check 7.5 MB/s. The earlier logos + recursive descent was ~21-27 MB/s
  on a different corpus (737 files), so there is a regression. Callgrind: ~30% malloc/free,
  `logical_lines` 7.6%, `scan` 12.5%, TokenKind clone/drop 5%. The `Word` DFA is not the bottleneck.

## Left

1. Lower the AST variants BC accepts that `semantic.rs` reports through `unsupported()`: ON ERROR RESUME NEXT,
   MethodCall, FormDeclaration, REDIM PRESERVE, OPEN FOR ISAM, OPEN ACCESS/LOCK/LEN/RANDOM, PALETTE USING,
   BYVAL/SEG args, END/SYSTEM exit codes. Keep debug lines and metadata.
2. Oracle gaps/leaks: `IF 1E THEN` / `1D` (BC accepts), VBDOS `CALL pp(CSNG(32768))`, bare method words,
   a TYPE field named EXPLICIT/REM, label corners, ndarr "Expression too complex", jpeg Math overflow.
   Areas not yet probed: PRINT USING, OPEN/file-I/O combinations, DATA/READ, DEF FN forms, ON ERROR,
   DECLARE ALIAS/CDECL/BYVAL, VBDOS forms and methods.
3. Speed: borrow lines in `logical_lines`, avoid per-identifier uppercase Strings, merge the `next_lists`,
   `without_volatile` and `kinds` token passes into one; remove `front/lexer.rs` and the `logos` dependency
   if the word DFA stays in `scan.rs`.
4. The readme (`crates/frontends/qb/lalrspike/readme.md`) should mention the Quickr port and the removal of the
   old parser.

## Red tests (also red on plain `origin/feat/parity`, checked in a clean worktree)

31 `llrm-qb` + 2 `qbfront` parser tests. They assert on the optimized output of small BASIC programs:
- POKE/PEEK/DEF SEG, about 13: `b$seg` and array-descriptor loads are no longer hoisted out of the loop
  (`test_hir_part3` `a_poke_*`, `test_text_memory_*`, `test_element_stores_*`, `peek_and_poke_*`).
- Counted loops, about 6: counter strength reduction / settled counters / registers parked for counters.
- Integer division, 3: constant-divisor reciprocal, nested division.
- Others: ON ERROR ordering, port poll and WAIT mask, OMF fixup, `test_the_first_dimension_is_not_rank_checked`.
- qbfront `a_power_of_a_variable_base_covers_the_whole_domain`: second assert (B$SERR count) fails.
Fixed on this branch (stale tests): `log_exp_and_fix_...` (fln/fexp) and `flat_target_varptr_is_a_whole_address`
(constant DEF SEG), and `test_a_cell_nothing_writes_in_a_handled_sub_folds` (`Show` is reserved in VBDOS).
The poke family's bisect result is first bad 4f82e22e1 (MemorySSA queries ask the AA stack with TBAA).

## Main commits left out of the rebase

#2340 (stamp: a write moved to an argument stays a write), #2362 (measure: build cache cap), #2374 (profit:
block weight saturates), #2387 (ci: merge ref carries main's workflow). They are on `main` but not on
`feat/parity`; replaying them conflicted (`llrm-transforms` interprocedural), and they are not part of this work.

## Unpublished local state (the old session's container)

- Local branch `backup/pre-parity-rebase` (pre-rebase history; the same commits are what `origin/spike/lalr-nib-qb`
  held before the force-push, a6bdd277c).
- HIR snapshots (before and after the old-parser removal), verdict snapshots, the BC oracle runner scripts
  and the Quickr sample programs were scratch files; none are in git.
