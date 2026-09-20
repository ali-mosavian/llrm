# qbopt

See [agents.md](agents.md). It carries the goal, the architecture and the
six rules, and this file exists only so that either name finds them.

The six rules, because they are the ones most easily forgotten mid-task.
The first three are global; the last three are this project's own, and all
six get restated at the end of a reply here:

1. **Say it and stop.** The minimum needed to understand, in every word
   written -- comments, commit messages, docs, tests, replies alike.
2. **Doubt the measurement before the subject.** A result that contradicts
   what is known about the thing measured means the instrument is wrong.
   Never conclude "there is nothing here" from a number common sense says
   should be large.
3. **Every issue found and fixed gets a regression test**, in the same
   commit as the fix, and written so that it fails before the fix goes in.
   A test never seen to fail is evidence of nothing.
4. **Dump every step and pass to a file and diff them.** Do not reason
   about where it went wrong -- `tools/stages.py` knows. Diff the MIR
   between passes, not the emitted code at the end.
5. **Every pass takes MIR and returns MIR**, and names nothing about the
   machine. Only lower, regalloc and peephole see machine form. What an
   idiom *is* -- a long pair, an absorbable call -- is the raise's answer,
   not a pass's, or the pass ends up knowing x86.
6. **Only general solutions, never an edge-case patch.** A fix that names
   one combination -- one pair of spaces, one routine, one demo -- means the
   mechanism is wrong. Name the general rule it is an instance of first; if
   there is none, building it is the work. Deleting code is the evidence it
   landed.
