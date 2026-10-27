> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopfilter.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopfilter.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopfilter.rs sha256:7195b2682eabd34b33089e587e27acdb6b79930e5032b449260c361012a335d3 -->

# loopfilter

Which loops a loop pass visits: the loop pass manager's worklist, for the
passes that walk every loop of a function.

## How it works
LLVM's `LoopPassManager` runs a list of passes on a worklist of loops. A
pass that makes loops (`LoopFullUnroll`) hands them back
(`LPMUpdater::addSiblingLoops`) and the passes after it run on those loops
alone. Ours are whole-function passes, so a slot that wants only some loops
sets a scope: `only` names the headers, and each pass drops the loops
outside them from the list it walks with `retain`. Without a scope every
loop is visited, as before.

## Example
After a full unroll made the loops headed by `b7` and `b9`, the slot runs
`indvars` inside `loopfilter::only(&[b7, b9])`: its steps see those two
loops and no others.

## Used by
`indvars` (its steps), `loopunrollpass` (the loops it makes), `pipeline`.
