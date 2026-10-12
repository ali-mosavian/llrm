# Finding a miscompile in a DOS program

How QCport's rich-route miscompiles were found: narrow the fault to one
module, stop the program where it goes wrong, then find the pass. The worked
example is snd_mix.c, which wrote 107k times into the VGA BIOS ROM with sound
on.

## 1. The runner: dosrun

`dosbox-x` built from the fork's `dosrun` branch (`./build-dosrun.sh`) boots
once and runs each job, from stdin, in a forked child. It streams JSON events
to the file descriptor `DOSRUN_FD` names. A job is DOS command lines plus
directives, ended by `.`:

    printf ':ms 60000\n:write C0000-C7FFF\nmount w %s\nw:\nQCPORT.EXE start.qmp -ticks 300\n.\n' "$DIR" |
      DOSRUN_FD=3 SDL_VIDEODRIVER=dummy dosbox-x -nolog -conf job.conf 3>ev.txt >/dev/null 2>&1

`job.conf` is the program's usual conf with the `[autoexec]` section cut off
and `core=normal`: the checks below run in the normal core only.

| Directive | Does |
|---|---|
| `:ms N` | Stops after N emulated milliseconds: `end` reason `limit`. A hang shows as `limit`, not as a job that never returns. |
| `:write A[-B]` | Stops at the first write into linear A..B (hex, inclusive). |
| `:nowatch` | Turns off the automatic stops, for a program that means to run code where they would object. |
| `:keep` | On a clean exit, keeps the machine: later jobs fork from its state. |

QCport needed `:ms` and `:write`. The source of truth is the comment at the
top of `src/misc/dosrun.cpp`.

## 2. What the fork adds to DOSBox-X

### Automatic stops

The normal core checks where execution lands after every transfer and on
entering a new page, in real mode, and stops the job with a `crash` event at
the first wrong place:

- **Unowned memory:** a free DOS block, an MCB header, video memory, or
  conventional memory past the end of the MCB chain. The interrupt table and
  BIOS data area are never code.
- **The program's own data:** memory its debug info or link map lays out as
  a data class, or its load image outside any code segment.
- **Empty memory:** four bytes of `00` or of `FF`, which no code starts with.
- **An unhandled fault:** #DE, #BR, #UD or #NM whose vector is still the one
  the booted machine had, so the program never meant to handle it.
- **HLT with interrupts off**, which nothing wakes.

A jump through a bad pointer, a return to a wrecked address and a call
through a corrupt vector all land in one of these. `:nowatch` turns the
checks off.

`:write A-B` is the same stop for data. Every write the core makes goes
through one check while a range is set. The stop comes before the write
lands, with the address, size and value in a `write` event.

### The call stack and the transfer ring

Each stop is followed by a `state` event: registers, and three views of how
execution got there.

- `stack`: frames by stack slot. A frame lives while its return address is
  still on the stack. This is wrong once the program has overwritten its
  stack, or when code runs on a stack of its own.
- `calls`: the calls still open. Every transfer goes into a 10,000-entry ring
  as a call, a return or a jump. A call records the address it returns to; a
  return pops back to the call that left its target. So the chain is what
  actually ran, whatever the stack now holds. In snd_mix it held across the
  sound IRQ's switch to its own 512-byte stack.
- `bad_returns`: the last 16 returns to an address no open call left. The
  first one near the stop is usually the corruption. Interrupt returns whose
  entry went unseen, and emu87's returns past its patched bytes, show up as
  noise.

A `trace` event then carries the ring, oldest first, with SP after each
transfer. `end` gives the reason (`exit`, `crash`, `write`, `limit`, `wall`)
and the program's executed instructions and memory operands.

A stop at an address that moves between runs is an interrupt, not the code
under it. One QCport crash stopped at a `sahf` in one run and elsewhere in
the next. The ring showed a "call" that dropped SP by six bytes (flags, CS,
IP) into the startup code's handler: a null far pointer had written the timer
vector.

### Symbols

At each program load the fork reads the program's own debug info:

- CodeView, TDINFO or Watcom info appended to the EXE; or
- a `.TDS` beside it; or,
- for an EXE only, the `.MAP` beside it (same name, `.MAP`).

A `.MAP` gives publics only, no statics, sizes or lines, so it is the
fallback. It also supplies the segment classes the data check uses. Without
any of them, a report has only `segment:offset` and linear addresses.

Every address in a report carries the linear address, the nearest symbol at
or below it as `symbol+offset`, and the source file and line where the debug
info has them. Where only publics are known -- a `.MAP` alone, or an object
with no debug info of its own, as llrm's are today -- the nearest symbol is
the last public before the address, so code in a static function reads as
an offset into the public before it. `_snd_mix_sum+0x23D` below is the static
`snd_mix_chan`. Name it from a listing (`llrm-c -S`).

## 3. Which module: swap llrm objects into the BCC build

Start from the program built entirely with BCC, and replace one module at a
time with llrm's. The private script `qcmix.sh MOD...` links BCC's objects with MOD's
llrm objects, runs 300 ticks and prints polys per frame. The oracle is the
frame: BENCH.BMP byte-identical to BCC's, compared against a private copy of
the reference, since shared runs overwrite theirs. With sound on,
`qcrom.sh MOD...` runs 60 s and prints DOSBox's `write ... to rom`
count; 0 is correct.

Run all modules at once, each in its own directory, then bisect the ones that
fail:

    qcrom.sh snd_mix        # llrm's snd_mix alone in the BCC build
    112320                            # ROM writes; BCC's own build writes 0

## 4. Where it goes wrong: the break

Run the failing mix under dosrun with a `:write` range covering the damage:

    :write C0000-C7FFF

The job stopped at the first ROM write:

    {"ev":"write","address":"C0AA4","size":4,"value":"7F000000",
     "at":{"at":"3608:9265","symbol":"snd_mix.c:_snd_mix_sum+0x0000023D"}}

`calls` gave `snd_mix_paint` calling a static function after `snd_mix_sum`,
which the listing names `snd_mix_chan`. The registers gave the rest:
`es:bx` = `B87F:82B4`, the channel pointer itself, and `esp` = `0x172`, a
tiny stack. QCport's `dsp.asm` runs its IRQ callback on a 512-byte stack of
its own with DS = DGROUP and SS not. `llrm-c -S` showed the caller reading
the channel array through `lds bx, dword ptr ss:[di+10]`: near data through
SS. BCC's `-S` listing of the same function reads it through DS.

## 5. Which stage: dump and diff

Compile the one module with every stage dumped:

    llrm-c -Os -I ... snd_mix.c -o snd_mix.obj --dump dump/snd_mix

`dump/<module>/NN-<pass>.ll` is the module after each module-level pass, and
the numbered directories hold each function's pipeline, one file per pass
that changed it. Extract the function from two adjacent dumps and diff them;
the first dump that differs from what the source means is the pass. If the
MIR is right to the last dump, the fault is in the machine phases:

    ISEL_DUMP=1 llrm-c -Os -I ... snd_mix.c -o snd_mix.obj

prints the LIR after each machine phase (FloatAssign, FloatAlloc, RegAlloc,
ParallelCopy, Peephole, LoopSlots, Scheduler). For snd_mix the MIR was right:
the fault was the machine model's promise that the stack is in the data
group, now `-mstack-is-data`. An r_walk.c miscompile, a reload placed
between `fnstsw ax` and `sahf`, was first present in the RegAlloc dump.

## 6. The regression test

Reduce the module to the smallest source that still shows the fault, and
record it with Watcom's front end as the fixture's `.cgs`. For snd_mix that
is `tests/fixtures/c/nearviads.c`, and the test asserts the symptom in the
listing: no `ss:`, `lds` or `mov ds,`. Watch it fail before the fix.
