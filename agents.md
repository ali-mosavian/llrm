# Working on llrm

For anyone changing llrm, human or tool: the project's rules and its method. What llrm is and how to build it:
[readme.md](readme.md). Where things are: [docs](docs/readme.md).

llrm, the Low Level Real Machine, is a compiler suite for 16-bit real-mode DOS aiming at GCC/LLVM-quality code.
Source frontends (QuickBASIC-family, C, its own language Nib) raise to HIR, then to MIR; one optimizer improves
MIR; one x86 backend lowers, allocates and writes objects. A small input route also raises objects BC wrote:
[docs/frontends/bc.md](docs/frontends/bc.md).

## Layout

- The crate is `llrm`; every command is `llrm-*` ([the programs](readme.md#programs)).
- Frontends are crates under `crates/frontends`; MIR, the optimizer and the backend are under `crates/ir`,
  `crates/opt` and `crates/backend`; one crate per target and object format is under `crates/target`.
  [crates](docs/reference/crates.md) lists them all and [source layout](docs/architecture/source-layout.md) says
  where the code of each stage is.
- Scripts and tools: [tools](docs/reference/tools.md). Format with `tools/fmt.sh`; plain `cargo fmt` is not
  enough.

**Writing rules -- repeat them after every iteration.** Keep it short, to the point and coherent. No rambling.
This applies to all writing: replies, docs, comments, tests, and git commit messages in Conventional Commit form.

## Measurements -- the second rule

**Doubt the measurement before the subject.** A result that contradicts what is known about the thing being
measured is evidence the instrument is wrong, not a finding. Never conclude "there is nothing here" from a number
that common sense says should be large.

- Several measures agreeing on zero is one broken assumption, not several facts.
- A question shaped so the subject's own style cannot answer it always returns zero. Ask what the subject
  actually does, not what the textbook name is.
- Verify against the raw thing -- read the assembly, the bytes, the output -- before writing a conclusion down.
  One look beats any amount of analysis.
- Derive a target by hand on both sides. An estimated one flatters whatever it measures.
- Cost hidden behind a call, an interrupt or a helper reads as free unless the measure is told about it.
- Never explain an implausible result with a story. That is how it survives.

## Regression tests -- the third rule

**Every issue found and fixed gets a regression test, in the same commit as the fix.** Not "should"; the fix is
not finished without it.

A bug that reached the gate got there because nothing smaller was looking, and nothing smaller will be looking
next time unless the fix leaves something behind.

- **Write the test so that it fails first.** Revert the fix, watch it fail, restore the fix. A test written after
  the fix and never seen to fail is evidence of nothing.
- **Test the symptom, not the patch.** The wrong output is the fact; which branch of which helper returned early
  is this week's shape of it. Assert on the emitted code or the program's answer where you can.
- **Instruments count.** A broken measurement is an issue like any other and gets a test like any other -- see
  the second rule. `tools/test_scoreboard.py` exists because a scoreboard summed what it counted before a compile
  failed.
- **Say what it cost.** The docstring names the program that printed the wrong number and what it printed. That
  is what makes the test readable in a year, and it is cheaper to write while it is still understood.

## Debugging -- the fourth rule

**Dump every step and every pass to a file, and diff them.** Do not reason about where the thing went wrong.
`--dump DIR` (or `LLRM_MIR_STAGES=DIR`) writes the MIR after each pass, `DIR/NN-<pass>.ll`; the first adjacent pair
that differs from what the source means names the pass. [Debugging](docs/working/debugging.md) walks through it.
`tools/baseline.sh OUT` dumps a whole corpus, and `diff -r` of two such trees judges a representation change.

- **One file per stage, then diff.** A wrong answer at the end says nothing about which stage produced it. Two
  adjacent dumps say exactly.
- **Diff the MIR, not the emitted code.** Reading the object means inferring backwards through layout, allocation
  and emission. The op list either side of a pass is the answer rather than a clue to it.
- **Project away the noise.** A raw diff of renumbered values is unreadable; dump the part that matters and the
  structural change is one line.
- **Build the dumper before you need it.** If no tool shows the step in question, write it first.

## The MIR boundary -- the fifth rule

**Every pass takes MIR and returns MIR, and names nothing about the machine.** Only lower, regalloc and peephole
see machine form. There is no LIR optimisation tier: if a pass has done its job on MIR there is nothing left for
one to do.

[docs/architecture/split.md](docs/architecture/split.md) is the whole of this rule: where the line is, what each
side may say, and why it is not negotiable.

- **Recognition is the raise's job.** What an idiom *is* -- a long pair, an absorbable runtime call -- is answered
  where the body is built. Answered later it is answered by a pass that must know about x86.
- **A pass may not name a register.** Lowering and the allocator are the only readers of where a value lived.
- **opt runs on one body, not through emission.** A pass that re-raises from bytes loses the SSA the one before it
  built and every marker with it.
- **A machine detail that survives into a pass is a defect in the raise**, not a thing to work around there.

## The general mechanism -- the sixth rule

**Only general solutions. Never an edge-case patch.** A fix that names one combination -- one pair of address
spaces, one runtime routine, one demo -- is a symptom of the mechanism being wrong, not a fix for it.

- **Name the general rule the fix is an instance of, before writing it.** If there is no such rule, the work is to
  build it. Say so and cost it out rather than adding the arm.
- **Prefer collapsing special cases to adding one.** Five partial answers to the same question are worse than one
  whole answer, however reasonable each was on the day.
- **Deleting code is the evidence.** A refactor that removes a special case landed the general form. One that
  only adds did not.
- **An assumption gets stated once, by name.** What makes it safe is that it is written down in one place with
  its justification, not spread implicitly across a dozen match arms.

## Shared proofs -- the seventh rule

**A fact about the program is proved once, by one analysis, and every pass asks it.** A pass that derives its own
proof -- its own stride walk, its own escape scan, its own trip bound -- disagrees with the next pass's version on
the first case either missed. Design by SRP, DRY, Open/Closed and separation of concerns:

- **One owner per fact.** Escape, provenance, trip counts, strides: each has one analysis. A pass consumes it; it
  never re-derives it.
- **A language's promise is stated by its frontend, in the IR.** That an access stays in its object, or a value is
  in range, is marked where the source says it -- not assumed by an analysis for every frontend.
- **Machine facts come from the target.** An analysis that needs an object size or address width reads it from the
  IR or the target, never a literal.
- **New cases extend the analysis, not its callers.** When a pass needs a fact the analysis cannot give, the
  analysis grows; the pass does not.

## The docs -- the eighth rule

**A change updates the doc of the thing it changes, in the same PR.** A pass, analysis, flag, level, tool, target or runtime that changes behaviour changes its doc with it: a pass's `//!` header is its doc (purpose, how it works, example, where it runs), and `python3 tools/docs/docsbuild/build.py --sync` output is committed with it (run `tools/fmt.sh` first: it rewraps header comments, and the page follows the final text). The stale-page and missing-section tests fail the PR where they can; where they cannot, the reviewer asks.

## The goal

**Produce what a modern optimising compiler would have produced.** Integers, longs, floats, array addressing,
loops -- all of it, judged against what a good backend would emit for the same source, not against what another compiler emitted.

A hand-derived optimal listing for each suite program is the target. Done means **every program within 1.5x**.
[Measurement](docs/working/measurement/readme.md) says how it is measured and where it stands.

## Code

- **Names say what the thing is, in as few words as do that.** `value`, not `v`; `from_memory`, not `m`. Not
  `the_value_being_encoded` either.
- **The code explains itself, or it is rewritten until it does.** A comment that has to say *what* the code does
  is a naming or structure problem.
- **DRY, SRP, open/closed.** One fact in one place -- two parallel switch statements over the same cases will
  drift, and a test asserting they agree is a patch over a structure that should not allow the disagreement. One
  function, one job. Extending the pass with a new idiom should mean adding a case, not editing the ones already
  there.
- **Functions are pure.** No mutating shared or global state, and no module-level code that does work.
  Module-level *constants* are fine and wanted -- but a table that has to be computed is computed by a function
  that returns it and bound once, and a script's body lives in `main()`.
- **Functions over types.** A type earns methods only when behaviour and state travel together.
- **Prefer an iterator chain where it fits.** A loop that only pushes one value per iteration is a
  `map`/`filter`/`collect` not yet written as one.
- **Comments carry a fact that is not in the code**: a measurement, the reason a case is refused, something a
  language or runtime does that nobody would guess. Narrating the next line does not qualify. Code lines go to column 120, comments to
  80; `tools/fmt.sh` enforces both.
- **Tests sit beside their module** (`*_tests.rs`, or `#[cfg(test)]` in it). Python tools' tests sit beside the
  tool, as `test_*.py`, written as pytest functions.
- **Docs are committed as GitHub shows them.** Figures are inline `mermaid` fences in the shared style
  ([figures](docs/working/figures.md)). After editing a `//!` header, run
  `python3 tools/docs/docsbuild/build.py --sync` and commit what it writes.
- **While working, run only the tests for the files being changed**: `cargo test --release <filter>`. See
  [testing](docs/working/testing.md).

## Writing

- **Commit messages are conventional commits**: `type(scope): description`, and a body. The subject says what
  changed; the body says what it was and why it had to. Both short and to the point, neither rambling. No
  `Co-Authored-By` trailer.
- **Replies in the session are held to the same rule**, and it is the one most often missed. Say the thing and
  stop: the finding first, no preamble, and no closing paragraph recommending what to do next unless a decision is
  open.
- PR bodies, docs and comments, likewise.

## Method

- **A fix starts with a failing test.** Reproduce the bug, watch the test fail, then fix it -- a test only ever
  seen passing proves nothing. This is about fixes. It is not TDD in general: new code may be written first and
  covered afterwards, as long as the cover is comprehensive.
- **Test behaviour, not implementation.** Tests should survive a refactor that changed no behaviour. One that does
  not is a test to rewrite.
- **When careful measurements of the source all come back clean and the program still misbehaves, stop measuring
  the source and check what was actually built.**
- **Consult an Opus-model agent to review a design or diff, and whenever stuck.** A review is big-picture, not
  adversarial. Ask it:
  - Is this at the right abstraction level? Does it generalize to cases not yet seen?
  - Does it follow DRY, SRP, Open/Closed and separation of concerns?
  - How does it fit the bigger picture? Does something need re-architecting?
  - What will a skilled engineer think of it in three years?

  Give it the plan or diff and the files it is grounded in. Resume the same agent across rounds of one question.
  If Opus is unavailable, fall back to Fable. Opus reviews and unblocks; it does not write the diff.
- **Before every push, run `tools/prepush.sh`.** It runs the gate's own cheap checks (format, generated docs, the source scans, a conflict-marker and whitespace check of the diff,
  `no_local_paths`, `target_facts`, `docs_links`) and prints one verdict line first: about 15 s with a warm target, two minutes
  cold. It is not the gate: no build, tests, benches, oracles or perf.
- Report what was measured, not what was expected.

Process rules (goal, PRs, landing, measuring, safety): [docs/working/pr-rules.md](docs/working/pr-rules.md).
