# Working on qbext

qbext replaces parts of the BASIC runtime and rewrites the code BC emits.
Everything here is self-modifying code aimed at three compilers at once, so
the ways it goes wrong are not the ways ordinary code goes wrong. This is
what has been established the hard way; most of it was paid for once already.

## The goal

Replace long runs of long operations with correct and optimised 386 code.
"Optimised" means real liveness and register allocation, so that the

    load, load, call, store, store

BC emits for *every single* long operation collapses into

    32-bit load, 32-bit op, 32-bit op, ..., 32-bit store

The prize is the store/reload/call traffic between operations, not the width
of any one of them. A pass that only widens pairs in place is leaving most of
it on the table.

## Scope

QB 4.5, PDS 7.1 and VBDOS. **Compiled mode only** -- under the QB/QBX editor
the call sites belong to the interpreter and are not ours to write.
`chkCompiled` is the gate and every entry point has to pass it.

## Rules

- **Check BC's output on all three compilers, and with the flags that
  matter.** They do not agree, and building against one of them is how you
  get a pass that fires on a third of what it should. `/G2` against `/G3`,
  `/FPi` against `/FPa`, and `/D` all change what comes out.
- **DOSBox cannot tell you what anything costs.** It charges per instruction
  and models no latency, so it cannot see the difference between an `imul`
  that is waited on and one that is not. Use `tools/cycles` for 486, P5, P6,
  K5, K6, K7 and Core, and treat those as a ranking rather than as cycle
  counts -- they are published latencies, not measurements.

  This is not a small correction here. BC's two 16-bit halves are
  *independent* chains, one through `ax` and one through `dx`, and an
  out-of-order machine already runs them together. Widening puts everything
  through one register: it halves the instruction count and leaves the
  dependency chain exactly where it was.

      speedup      486     P5     P6     K5     K6     K7   Core
      alone      1.81x  1.87x  0.98x  1.03x  1.03x  1.04x  1.01x
      back to back  "      "   1.44x  1.83x  1.83x  1.83x  1.56x

  So the win is real on the in-order parts and depends entirely on
  surrounding work on the rest. DOSBox reports the in-order answer, and
  quoting it alone overstates the case on anything after a P5.
- **Be hands on with DOSBox.** Drive it, read the screen, take screenshots.
  Do not infer from a batch file that finished.
- **No type suffixes in BASIC declarations.** `As Long`, `As Integer`,
  spelled out.
- **Credit boostqb.** The call-site patching technique, the eight-byte cdecl
  window and `chk_compiled` are v1ctor's, from boostqb, copyleft 2000.

## What BC actually emits

Measured across all eight builds of the same source: the core shape is
**identical everywhere**, and only the call sites differ.

    mov ax,[X]   mov dx,[X+2]         load a long into ax:dx
    <op> ax,[Y]  <op> dx,[Y+2]        and again for each further operation
    mov [Z],ax   mov [Z+2],dx         store it

- BC **does chain** in registers -- three operations between one load and one
  store is ordinary. An earlier note here claimed it stored every
  intermediate; that was an artefact of a generated test with one operation
  per statement, not a fact about BC.
- Intermediates spill to **bp-relative** stack slots (`89 56 EA`, `8B 46 E8`),
  and the spilled store pair comes out **dx first, then ax** -- the reverse of
  the direct form. A matcher that only accepts ModRM `06` and `16` sees a
  spill as opaque and cuts the run in half.
- After a call to the runtime the result is **already in dx:ax** and BC
  carries straight on with operation pairs. A run can begin at a call return
  as well as at a load.
- **The long argument push differs by compiler.** VBDOS with `/G3` uses one
  32-bit push, `66 FF 36 <a>`, and the call sequence is fifteen bytes. PDS
  7.1 and QB 4.5 push two words, high half first so the low word lands at
  the lower address -- `FF 36 <a+2>` then `FF 36 <a>` -- and it is
  twenty-one. Both are absorbed now, and the difference is not only the
  pattern: twenty-one bytes leaves room fifteen does not, so `MOD` fits on
  PDS and QB 4.5 and often will not on VBDOS.
- **The first long pushed is the divisor, the second the dividend.** Read
  off what BC emits, not assumed. `B$DVI4` takes its dividend from
  `[bp+6]`, which is the last pushed. Multiply is commutative and would
  have worked either way, which is exactly why it is worth knowing before
  divide is attempted.
- `/D` roughly triples the far calls, interleaving runtime checks through
  everything.

## Things that will bite

- **cdecl is why the call-site patching fits.** The caller cleans up, so BC
  always emits `9A` + `83 C4 nn` -- five bytes plus three, and eight is enough
  for all six peek/poke forms. Under `pascal` there are five, which fits one
  of them.
- **The FP emulator patches its own call sites.** `int 34h`..`3Dh`, 58 of them
  in 15.8K of BC code, rewritten in place the first time each runs. Move code
  out from under one and the emulator patches an address that has shifted.
  Blocks containing one are untouchable.
- **DOS relocates the image at load time.** The file and the loaded program
  differ at every relocation -- 252 sites in one small test. Comparing a host
  decode of the EXE against a runtime decode is only valid below the first
  one; past that the two are reading different programs.
- **"Transfers control" is not "ends a basic block".** Calls and software
  interrupts come back. 49 of 102 blocks once ended at a far call that was not
  a block end at all, which halves the length of everything liveness can see
  and puts a boundary exactly where the store/reload worth removing lives.
- **`FF /4` and `/5` are indirect jumps and do end a block** -- that is what
  `ON GOTO` and `SELECT CASE` compile to -- and the opcode tables give `FF` no
  QFLOW at all. Their targets are not computable, so at level 3 they have to
  read as "assume everything live".
- **A call destroys the flags.** Nothing here preserves them, and `B$CPI4`
  returns its answer in them through `lahf`/`sahf`. A `jng` sitting after a
  long operation is usually reading the *call's* flags, not the operation's.

## The host tools cannot see an absorbed call

`tools/qbe/lift.py` reads the EXE on disk, where the segment word of a far
call has not been relocated yet -- DOS fills it in at load time. So the
address comparison that recognises `B$MUI4` fails on the file and succeeds
at run time, and the host analysis reports no absorbed calls however many
there are.

Everything else about the lift is visible from the file. This one thing is
not, so a coverage number from `price.py` or `fixups.py` is a lower bound
wherever calls are involved, and the figure to quote is the one qbeStat
reports from inside.

## The window is a contract

`qbeRun`'s `nb` must stay inside the caller's own module. Past the end of
`BC_CODE` is the BASIC runtime, which does long arithmetic in exactly the
shape this pass recognises -- so it lifts it, rewrites it, and the program
then runs a runtime that no longer works.

Nothing here can find that boundary. Segments are not delimited at run
time, and runtime code does not look different from module code. It is the
caller's to get right, and getting it wrong does not fail loudly: asking
for 30000 bytes of a module with 1662 of code hung PDS 7.1 and quietly
survived on VBDOS and QB 4.5.

## The 66h prefix, and the floor it puts under all of this

Every widened instruction carries a 66h, because the code segment is
16-bit and in real mode there is no making it otherwise. On the in-order
parts a prefix costs about a decode clock, and that is a tax the 16-bit
form does not pay:

    and cx,[X]     16-bit      2 clocks on a 486
    and ecx,[X]    widened     3

Which decides whether any of this is worth doing on a 486:

    speedup, benchmark region      486     P5
    prefix charged one clock     1.00x  1.03x
    prefix free                  1.72x  1.81x

**This is the largest open question in the whole exercise and it is not
settled.** Intel's i486 documentation says one additional clock per
prefix; how much of that overlaps with the previous instruction in
practice is what decides it. DOSBox charges per instruction and so
reports the optimistic column, which is why the benchmark says 1.24x.

`tools/cycles/timings.py` now charges it, so the pessimistic column is
what the tools report by default. Setting `PREFIX` to zeroes gives the
other. Neither is a measurement -- that needs real hardware, and until
somebody does it the honest statement is that the instruction count
halves and the wall-clock effect on a 486 is between 1.0x and 1.7x.

## What widening changes, exactly

Measured over 1125 cases in `tools/qbe/flageq.py`:

| | disagree |
|---|---|
| values | **never** |
| CF, SF, OF | **never** |
| ZF | 18.7% |
| PF | 37.2% |
| AF | 12.4% |

BC's pair leaves the flags of the **high half**; one 386 instruction leaves
the flags of the **whole result**. `1 AND 1` is the clearest case: the high
half is zero so BC sets ZF and the widened form does not.

Two things follow. Only ZF, PF and AF are at issue, so `adc` and `sbb`, which
read CF alone, do not block a run -- and `inc` and `dec`, which leave CF alone
but write the other three, do kill them. And **`dx` must be dead or put
back**: BC leaves the halves in `ax` and `dx` and goes on using both, while
one 386 instruction writes only `eax`. Reconstructing it costs `mov edx,eax`
and `shr edx,16`, which fits a one-operation run and not a zero-operation one.

## Tests

    python3 tools/qbe/unit.py           the pieces, on bytes built in the test
    tools/qbe/regress.sh [vbdos|pds|qb45]   whole programs, in DOSBox
    python3 tools/qbe/dectest.py EXE MAP    the decoder against ndisasm
    python3 tools/qbe/price.py EXE MAP      what a rewrite costs, in cycles

Every case in `src/test/qbeopt.bas` is a shape the pass got wrong once, and
the regression runs the same arithmetic twice -- as BC compiled it, then
after the rewrite -- so a difference is the pass. It cannot be done on the
host: the rewrite runs against loaded code, and DOS relocates at load time,
so the bytes the pass sees are not the bytes in the file.

The unit tests exist because the other two were both green while the
classifier reported "no half" for A1, while a pair copy was treated as
dead, and while `sizeof` and `encode` disagreed about a bp-relative store.
Agreement between implementations is not correctness when both were
written from the same wrong idea.

## Method

These are not style preferences. Nearly every bug in this directory was found
by one of them and by nothing else.

- **Write the test first and watch it fail, for the stated reason.** Not "add
  a test afterwards". The register model went in as a stub answering "reads
  everything, writes nothing", the thirty-two cases were written against what
  it should say, they failed, and only then was it filled in. A test that has
  only ever been seen passing has not been shown to test anything -- and
  twice here one passed because it was asserting the wrong thing.
- **Every bug found gets a case before it is fixed.** The list in
  `src/test/qbeopt.bas` is exactly that: nine shapes, each one something this
  got wrong once. Fixing without adding the case is how the same class came
  back three times.
- **Run all three compilers before saying it works.** VBDOS passing is not
  the answer; PDS hung for an afternoon while two of three were green, and
  that split is what made it look like a compiler bug rather than a bad
  argument. `tools/qbe/regress.sh vbdos|pds|qb45`, and both suites in each.
- **Check the thing is actually connected.** The register model built,
  tested and passed while `qbo$live` was still ignoring it, so nothing had
  changed. A green suite says the piece is right, not that it is wired in --
  measure the effect as well.

- **Two implementations, one on the host and one on the target, compared.**
  The prototype in `tools/qbe` and the assembly are checked against each other
  on the same program. Agreement on the count matters as much as agreement on
  the answer: x86 resynchronises after a bad instruction length, so two walks
  can end in the same place having disagreed in the middle.
- **Mutation-test every gate.** A gate that has only ever passed proves
  nothing. Teach it something false, watch the numbers move, put it back.
  Check the mutation actually applied -- a patch pattern with the wrong line
  endings matches nothing and reports success.
- **Prove the pattern occurs before writing the rewriter.** `qbeWiden` was
  written, shipped and believed for a long time while matching nothing in any
  real program, because nobody had asked it how often it fired.
- **When a number surprises you, read the instructions.** Twice here a
  measurement was wrong in a way only the disassembly showed: 11 runs that
  looked like they were having their flags read were reading a call's, and a
  decoder that looked broken was reading relocated bytes.
- **Run the control.** Before believing a rewrite broke something, run the
  same test with the rewriting turned off.

## Small things that cost an afternoon

- **`sizeof` and the emitter must agree to the byte.** If they do not the
  region overruns the code after it, and nothing says so until something
  crashes. `qbo$cost` drifted out of step twice -- once when bp-relative
  operands arrived and again when the absorbed calls did -- and both times
  a patch had silently matched nothing because a label had been split onto
  its own line. `src/test/qbeunit.bas` asserts it now by checking where the
  jump over the slack lands.
- **BC has about 46K to compile in.** `qbeunit.bas` reached 41K generated
  as a `memPoke` per byte and the compiler hung rather than saying so. The
  tables are DATA now, which is a tenth the size.
- **A single-line `IF ... THEN stmt` immediately before `ELSE`** closes the
  block early, and the error lands on a `NEXT` a long way below as "FOR
  without NEXT". Use block `IF` throughout in these tests.

- `off`, `base` and `val` are reserved words in BASIC. The error lands on the
  `DECLARE`, pointing at nothing that looks like a keyword.
- MASM writes a prologue for you. Writing your own gets two, and every stack
  offset shifts by two -- `qbeSelf` returned segment zero that way and the
  test cheerfully decoded the interrupt vector table. Force a frame with a
  `local`, the way `uglInit` does.
- `jcxz` reaches 128 bytes. Loop bodies grow.
- `cx` cannot index in 16-bit addressing. Only `bx`, `bp`, `si`, `di`.
- `qbe$len` returns its length in `ax`, over whatever the classifier just put
  there. Carry the classification across the call.
- A `proc` that walks `si` must restore it if the caller advances from a
  variable instead -- otherwise the caller's "did this advance?" test compares
  a value with itself, and the scan quietly stops after one block having
  reported no error.
