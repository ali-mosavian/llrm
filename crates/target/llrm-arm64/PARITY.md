# arm64: parity with the pin

## Deviations, numbered

| # | Where | Deviation from the pin | Exit |
|---|---|---|---|
| GM1 | `llrm-transforms/src/globalmerge.rs` | a global that another initializer, an alias or a landing pad names stays unmerged; external globals are not merged (MIR has no alias to a struct member and the writer emits none) | 11-12 |
| GM2 | `llrm-transforms/src/globalmerge.rs` | a global with metadata stays unmerged (the pin copies `!dbg`) | 11-12 |
| GM3 | allocation | stft_single/stft_fixed/nbody_fixed O3 run more instructions than release/next without the merge; the merged address adds are the pin's, the residue is spills and copies | 11-12 |
| SF1 | `sinkfold.rs` | sink-and-fold runs after phi elimination: a value qualifies when it and its operands are defined once (the pin: SSA) | 11-12 |
| SF2 | `sinkfold.rs` | register pressure at the sink target counts live virtual registers against the allocatable ones (the pin: per pressure set) | 11-12 |
| FS1 | `frame.rs` | a frame with a moving stack pointer keeps the combined stack bump (the pin splits it at 512 bytes too) | 11-12 |
