# Finding a miscompile in a DOS program

How QCport's rich-route miscompiles were found: narrow the fault to one
module, stop the program where it goes wrong, then find the pass. The worked
example is snd_mix.c, which wrote 107k times into the VGA BIOS ROM with sound
on.

## 1. The runner: dosrun

`dosbox-x` built from the fork's `dosrun` branch (`./build-dosrun.sh`) runs a
job from stdin, headless, and streams JSON events to the file descriptor
`DOSRUN_FD` names. A job is DOS command lines plus directives, ended by `.`:

    printf ':ms 60000\n:write C0000-C7FFF\nmount w %s\nw:\nQCPORT.EXE start.qmp -ticks 300\n.\n' "$DIR" |
      DOSRUN_FD=3 SDL_VIDEODRIVER=dummy dosbox-x -nolog -conf job.conf 3>ev.txt >/dev/null 2>&1

`job.conf` is the program's usual conf with the `[autoexec]` section cut off
and `core=normal`: every check below runs in the normal core.

| Directive | Does |
|---|---|
| `:ms N` | Stops after N emulated milliseconds: `end` reason `limit`. A hang shows up as `limit`, not as a job that never returns. |
| `:write A[-B]` | Stops at the first write into linear A..B (hex, inclusive): a `write` event, then the state as a crash. |
| `:nowatch` | Turns the crash checks off, for a program that runs code where they would object. |
| `:keep` | On a clean exit the machine is kept and later jobs fork from it. |

The ones QCport needed were `:ms` and `:write`.

## 2. What a stop reports

Without a directive, the job stops where execution lands somewhere code
cannot be: memory no one owns, a program's own data, empty memory (`00` or
`FF` bytes), an unhandled #DE/#BR/#UD/#NM, or HLT with interrupts off. The
events, in order:

- `crash` (or `write`): what stopped it, and where.
- `state`: registers and three views of how execution got there:
  - `stack`: frames by stack slot. A frame lives while its return address is
    still on the stack. Wrong once the program has overwritten its stack.
  - `calls`: the calls still open, matched by return address from the
    transfer ring. Survives a wrecked stack.
  - `bad_returns`: the last 16 returns that went where no open call left.
    The first one near the stop is usually the corruption. Returns from
    interrupt stubs whose entry was not seen, and emu87's patched returns,
    show up here as noise.
- `trace`: the last 10,000 transfers (call, ret or jump), oldest first, each
  with its source and target and SP after it.
- `end`: the reason (`exit`, `crash`, `write`, `limit`, `wall`), emulated ms,
  and the program's executed instructions and memory operands.

Each place carries the linear address, the nearest public symbol and, with
the program's CodeView info, the source line. The nearest public is the one
before the address, so a static function reads as an offset into the public
before it.

A stop at an address that moves between runs is an interrupt, not the code
under it. One QCport crash stopped at a `sahf` in one run and a different
instruction in the next; the `trace` showed a "call" that dropped SP by six
bytes (flags, CS, IP) into the startup code's handler. A null far pointer had
written the timer vector.

## 3. Which module: swap llrm objects into the BCC build

Start from the program built entirely with BCC, and replace one module at a
time with llrm's. `~/scratch/qcmix.sh MOD...` links BCC's objects with MOD's
llrm objects, runs 300 ticks and prints polys per frame. The oracle is the
frame: BENCH.BMP byte-identical to BCC's, compared against a private copy of
the reference, since shared runs overwrite theirs. With sound on,
`~/scratch/qcrom.sh MOD...` runs 60 s and prints DOSBox's `write ... to rom`
count; 0 is correct.

Run all modules at once, each in its own directory, then bisect the ones that
fail:

    ~/scratch/qcrom.sh snd_mix        # llrm's snd_mix alone in the BCC build
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
