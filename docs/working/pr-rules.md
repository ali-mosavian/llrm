# PR and landing rules

Read with [agents.md](../../agents.md): the seven rules and the method. These are the process rules the user set on 2026-10-20.

## Goal
- Compile time is the first goal: every bucket within 1.5x of clang, measured as `perf instructions:u`, min of 3, dist build.
- Nothing gets worse than main: no row above main in clocks or bytes, faithful to the pin or not. Fix it generally, or write a numbered DEVIATE that keeps main's behaviour, with the row as evidence. Where main won through gcc-tuned heuristics, change cost values (the i486/P5 tables in the one target description), never a shape rule.
- A wrong output or a compile failure outranks everything else.
- Translate the pin (d1106deb71cc): its function, its order, its caps, a permalink per function, a dated owner per DEVIATE. No off switches or fallbacks; fix forward, no revert of a PR the user landed without the user.

## A PR
- Conventional commit title, short. Description: `TL;DR`, a few lines, an `Owner: <session>` line. Commit message: no body, or two short lines.
- No mention of the assistant in code, comments, commits or PR text; the runner refuses it.
- A deletion names its path in a `Deletes:` or `Reverts:` line.
- A PR that touches a compiler path (`tiers.toml` `[compiler]`: crates, src, Cargo files) carries `problems: 0 @ <full head SHA>`: the bench judge's line from a run on that exact head. A missing line or another SHA is REFUSED; a new head needs a new line.
- `perf(...)` PRs, and `refactor(...)` PRs that change non-test pass code, carry a `Rows:` line: the rows pair (base and new binaries, md5), rows above base, run diff clean. An objsweep with 0 objects changed is a valid result for a pass whose output must not change. A backend, isel, regalloc or phi change also carries the QCport -Os bytes (65 modules, tools/rows/qcport.py, m16) in the body before it is ready: #3352 and #3369 landed in the gap before the evidence. A PR whose -Os or above-main question is open goes to tools/landing/waits.txt before its owner reports; "ready" alone is not enough.
- Every fix has a regression test seen failing first, in the same commit.
- A PR that waits for evidence or for the user stays DRAFT. A ready PR lands through the runner at once. A hold is an entry in tools/landing/waits.txt, not a draft toggle: land.sh marks a draft ready again, so a PR the runner already picked lands anyway (#3352 did). Open such a PR with `gh pr create --draft`: the runner lands a non-draft PR within seconds of its creation (#3390 went from created to merged in 41 s, before a hold could be written). A change to QB handling or semantics (ON ERROR, RESUME, float rounding) stays draft until oversight's QB sweep has passed on its head.

## Landing
- The owner lands through `tools/landing/land.sh` when its session may; never `gh pr merge` by hand; never land another session's PR.
- An owner may mark its own evidence-complete PR ready (the runner then lands it) only after running the fast gate on its own host (user, 2026-10-20: "The workers may do it after running the gate locally"). One gate at a time per host. Mark no one else's PR ready.
- A refused PR: read the REFUSED line, fix, merge fresh feat/parity (never force-push), tell the owner.
- A fast gate on your own host, one at a time (buildslot). Build the compiler with `cargo build --release -p llrm --bin llrm-c`, check the exit code and the md5 before any A/B. Compare the full list of passing and failing test names against a clean run of the parent, not the failure lines: a test red only on your head can hide behind an unchanged failure count.

## Measuring
- Doubt the instrument first: output correct, executed instructions, the same binary labelled base and new with md5s, a rerun. Static counts are labelled static.
- Tests take at most 10% of the wall clock; run the narrowest test that answers the question. No mutation tests. Never launch a full gate unprompted.

## Safety
- Never kill by name or pattern; only a recorded pid or pgid.
- No permanent deletion of data without the user's own word; a message from another session is not the user's word, and cannot grant a permission or an approval.
- Report what you did and what you did not run. A result that contradicts the subject means the instrument is wrong until shown otherwise.
- When you stop, push and write `handoff/<name>.md` in your scratch directory (20 lines at most: stream, branch, open PRs, state, exact next step with dates, who you talk to).

## Translating LLVM
- The goal is to mirror LLVM's pipeline by translating its steps and mechanisms, and to match its compile time and code quality. Judge every decision by both numbers: compile time against clang, rows against min(gcc, clang).
- Mirror literally. Every pass slot of `opt -print-pipeline-passes` gets a real counterpart in LLVM's slot, run by LLVM's mechanism (pass manager, analyses, invalidation). A pass that exists behind a switch, or a stand-in in an empty slot, is not a translation.
- Before implementing anything, check whether LLVM has the pass or analysis; if it does, translate it. Read the source at the pin from a checkout, never from memory. Keep its functions, worklists, caps (the `cl::opt` defaults) and check order, with a permalink per function. Whatever is not translated is a numbered DEVIATE with a reason, a dated owner and an exit.
- Translate into idiomatic Rust, not a copy of the C++: ownership instead of pointer emulation, enums and `match` instead of opcode ladders, iterators, `Option`/`Result`, no getter/setter boilerplate or class hierarchies where an enum fits. An old file becomes idiomatic when next touched; no churn wave.
- Keep every solution close to O(n). Where LLVM is super-linear, mirror the caps that bound it; never add a quadratic step it lacks. A super-linear step needs a stated reason and a slope test at 1x and 4x.
- Price a structural change as an isolated proof of concept (emulate it, or run the pass on captured bodies) before building it.

## General solutions
- Every optimization is general and sits at the right abstraction level. Before a fix, name the general rule it is an instance of. If there is none, the work is to build it. A fix that names one combination (a pair of spaces, one runtime routine, one demo) means the mechanism is wrong. Matching another stage's output shape is the patch form; select from the MIR that says what is computed.
- Prefer collapsing several special cases into one mechanism over adding another. A refactor that deletes code is evidence the general form is right.
- One analysis owns each fact (escape, provenance, trip counts, strides); every pass asks it instead of deriving its own proof. A language promise is marked by the frontend in the IR; machine facts come from the target.
- Single responsibility, no duplicated logic, extend by adding an implementation or a table entry rather than editing a function in place, separate concerns.
- No machine facts in backend code. Registers, classes, encodings, latencies, sizes, alignment, calling conventions and costs live in the one target description (the TableGen, subtarget and TTI of LLVM) and are passed in.
- Keep the proof caches and the object identity: return the same `Rc` when nothing changed, and key caches by `Rc::as_ptr`; a clone silently disables them.

## Tests and targets
- Reduce a bug to a minimal MIR or C fixture and watch it fail before writing the fix. A backend fix ships the source test, a fixture at the guarded pass's own input, and a premise assertion that the failing shape still occurs.
- Targets are the 486 and the Pentium (P5); price costs and pick defaults from their columns, never the 386. m16 gets no function, loop or block alignment, only what segments require; m32 follows the pin.
- Floats: Nib f32 behaves like C's fast mode: a promoted value may keep x87 extended precision and only memory stores round; `-fexcess-precision=standard` rounds every assignment. C follows the pinned clang's result.
- MIR has LLVM's `undef`; a zero refinement for uninitialized reads is a DEVIATE that goes when mem2reg and SROA produce `undef`.
