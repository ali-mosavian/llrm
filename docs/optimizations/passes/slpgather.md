> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpgather.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpgather.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpgather.rs sha256:9ecd468e18e757d63c9e7293f2256b5f76e0ce3e14c8a3780fbe9da521042028 -->

# slpgather

`SLPVectorizer`'s lane reuse and build-vector gathers: the lanes of a
bundle that repeat, packed once and read again through a shuffle mask
(`ReuseShuffleIndices`), and the vector a gather builds from its scalars.

## How it works
`tryToFindDuplicates` (l.9997) lists the distinct lanes of a bundle and the
lane each position reads; `EstimatePackPlusShuffleVsInserts` (l.10069)
keeps the bundle packed when the distinct lanes' inserts and the reuse
shuffle cost no more than inserting every lane. A gather is priced and
built by `processBuildVector` (l.22606): `TryPackScalars` (l.22861) puts
each distinct non-constant scalar at its first lane, a value in every lane
becomes lane 0 and a broadcast, `getBuildVectorCost` (l.14851) and
`getGatherCost` (l.21526) price the inserts, `finalize` (l.15683) the
shuffle of the reuse mask over the packed lanes.

DEVIATE SLP5 (owner arm64, exit 2026-11-24): no undef lane, no
`isGatherShuffledEntry` (a gather read from another entry's vector), no
alternate-opcode bundles, loads that scatter or stride; a bundle the pin
keeps unpacked with its repeats (`UseOriginal`) is a gather here; a count
of distinct lanes that is not a power of two is not packed.

## Runs
Under `slptree`, for each bundle that repeats a lane and each gather.

## Used by
`slptree`.

## Example
A bundle whose lanes repeat is packed once with its distinct lanes and read
back through a shuffle mask.

```text
bundle lanes: (%a, %b, %a, %b)
=>
%v = <2 x T> (%a, %b)
%r = shufflevector %v, poison, <0, 1, 0, 1>
```
