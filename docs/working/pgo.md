# The profile-guided llrm-c

A build of `llrm-c` guided by a profile of compiling real code takes 15-18% fewer instructions per compile than the `dist` build, with the same
output bytes. Measured on programs the profile never saw (d_faces, lru, bintree, queens at -O1, -O2 and -Os; the training excluded them):
instructions -14.6..-18.3%, cycles -6..-18%.

The pull request gate and every instruction-count budget stay on the plain build: an algorithmic change is judged the same with or without
this. The profile-guided build is for the shipped compiler and for the second column of the compile-time scoreboard.

## Building it

    rustup component add llvm-tools          # the llvm-profdata of rustc's own LLVM; another version cannot read the profiles
    python3 tools/pgo/pgo.py build --out DIR --profile-dir PROFILES [--siblings DIR_WITH_THE_OTHER_TOOLS]

`DIR/llrm-c` is the compiler. The first run makes the profile (an instrumented build, the corpus through it, a merge: about 15 minutes) and
the second build uses it; a later run reuses the profile while it is fresh. `QCPORT` (QCport's `src`) adds its render modules to the training.
`--exclude NAME ...` leaves programs out of the training: use it to measure on programs the profile did not see, never measure a program
that trained it.

CI: `.github/workflows/pgo.yml` runs on a push to main that changes the compiler. It builds both, requires `tools/identical.py` to find the
same bytes (every program and tests/run/c program at each level; the job fails otherwise) and uploads `llrm-c-pgo`.

## The profile

- What trains it: `tools/pgo/corpus.toml`: every bench C program at -m32 and -m16, at -O1, -O2 and -Os, and every tests/run/c program at its
  own flags. All in the repository: QCport's modules need its C library headers, which CI does not have. With `QCPORT` set (its `src`) they
  join the training locally. A compile of the corpus that fails fails the job: a corpus that does not compile trains less than it says.
- Size: about 30 MB. It is a CI cache keyed by the compiler tree's hash (`tools/bench/basecounts.py key`), never committed.
- Age: it describes the source it was made from. A function changed since gets no profile data, as does one the training never ran
  (a fresh profile already leaves about 6,000 of them: the nib and OMF front ends, error paths). The build prints `no profile data available
  for function ...`; the job summary shows the count and the first twenty: compare it with the fresh figure, not with zero. So it is reused for at most 50 commits to the compiler
  (`crates/`, `src/`, `Cargo.*`) or 14 days, and made again past either (`pgo.py stale`: 0 fresh, 1 stale, 2 none). A commit that touches
  none of those does not age it.
- A profile from a commit the clone lacks is stale.

## Measuring with it

The scoreboard's second column is `DIR/llrm-c` from the command above, run against the plain `dist` llrm-c on the same commit. Report
instructions (`perf stat -e instructions:u`) as the figure; cycles on a loaded host are noise unless alternated and the minimum of seven.
