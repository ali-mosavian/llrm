# The profile-guided llrm-cc

A build of `llrm-cc` guided by a profile of compiling real code takes about 15-18% fewer instructions per compile than the `dist` build, and
7-13% fewer cycles (user time follows the cycles, not the instructions), with the same output bytes. Measured on programs the profile never
saw, with the training of this directory's corpus (the repository's own programs, none of these four, no QCport at all); one tree, plain `dist`
against `dist` with the profile; cycles are the minimum of seven runs alternated with the other binary on a loaded host:

| compile | -O1 instr / cycles | -O2 instr / cycles | -Os instr / cycles |
|---|---|---|---|
| d_faces (-m32) | -14.7% / -9.2% | -14.8% / -7.6% | -14.0% / -7.3% |
| lru | -17.4% / -9.8% | -17.7% / -9.8% | -17.7% / -10.2% |
| bintree | -16.7% / -11.5% | -17.8% / -11.6% | -18.0% / -13.2% |
| queens | -16.7% / -11.5% | -18.0% / -11.6% | -18.2% / -12.7% |

The pull request gate and every instruction-count budget stay on the plain build: an algorithmic change is judged the same with or without
this. The profile-guided build is for the shipped compiler and for the second column of the compile-time scoreboard.

## Building it

    rustup component add llvm-tools          # the llvm-profdata of rustc's own LLVM; another version cannot read the profiles
    python3 tools/pgo/pgo.py build --out DIR --profile-dir PROFILES [--siblings DIR_WITH_THE_OTHER_TOOLS]

`DIR/llrm-cc` is the compiler. The first run makes the profile (an instrumented build, the corpus through it, a merge: about 15 minutes) and
the second build uses it; a later run reuses the profile while it is fresh. `QCPORT` (QCport's `src`) adds its render modules to the training.
`--exclude NAME ...` leaves programs out of the training: use it to measure on programs the profile did not see, never measure a program
that trained it.

CI: `.github/workflows/pgo.yml` runs on a push to main that changes the compiler. It builds both, requires `tools/identical.py` to find the
same bytes (every program and tests/run/c program at each level; the job fails otherwise) and uploads `llrm-c-pgo`.

## A release build

A release takes the compiler from the latest green `pgo` run on main: the artifact `llrm-c-pgo` (14 days), built from that commit with the
profile described below. Building it elsewhere is the command above with the profile cache: the key is `pgo-profile-` and the compiler
tree's hash (`python3 tools/bench/basecounts.py key HEAD`), and a commit with no profile of its own restores the newest older one
(`restore-keys: pgo-profile-`) and uses it while `pgo.py stale` says fresh (at most 50 commits to the compiler and 14 days). A cache miss
makes a new profile (about 15 minutes more); the first pgo run on main (39018948814, 27 minutes: 547 training compiles, 1,323 objects
identical) did exactly that. A cache saved by a pull request run is visible to that pull request only.

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

The scoreboard's second column is `DIR/llrm-cc` from the command above, run against the plain `dist` llrm-cc on the same commit. Report
instructions (`perf stat -e instructions:u`) as the count and the cycles beside it: the cycles are the smaller gain.
