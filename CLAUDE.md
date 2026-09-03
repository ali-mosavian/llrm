# qbopt

See [AGENTS.md](AGENTS.md). It carries the goal, the architecture and the
three rules, and this file exists only so that either name finds them.

The three rules, because they are the ones most easily forgotten mid-task:

1. **Say it and stop.** The minimum needed to understand, in every word
   written -- comments, commit messages, docs, tests, replies alike.
2. **Doubt the measurement before the subject.** A result that contradicts
   what is known about the thing measured means the instrument is wrong.
   Never conclude "there is nothing here" from a number common sense says
   should be large.
3. **Every issue found and fixed gets a regression test**, in the same
   commit as the fix, and written so that it fails before the fix goes in.
   A test never seen to fail is evidence of nothing.
