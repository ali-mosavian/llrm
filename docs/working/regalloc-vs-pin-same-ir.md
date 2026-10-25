# RegAlloc against the pin on the same IR

2026-10-25, d_faces -O2 m32. Ours: dist 898f99e494 callgrind. Pin: `llc -O2 -mtriple=i386 -mcpu=i486` on our IR after the mid-end (`LLRM_MIR_STAGES`, `30-switchconv.ll`), total 216.0 M. Call counts agree, so the gap is cost per call.

| Part | Ours Ir (calls) | Pin Ir (calls) | Per call |
|---|---|---|--:|
| trySplit | 39.2 M (114) | 17.1 M (104) | 2.1x |
| calculate_region_split_cost | 20.5 M (104) | 11.4 M (106) | 1.8x |
| grow_region | 11.9 M (412) | 7.3 M (410) | 1.6x |
| InterferenceCache update | ~8.2 M (8.8 K blocks) | 4.6 M (3.6 K) | ~1.1x per block |
| SplitEditor::finish | 10.5 M (65) | 4.1 M (60) | 2.4x |
| SplitAnalysis::analyze | 2.4 M (114) | 0.84 M (122) | 3x |
| split_live_through_block | 3.1 M (1147) | 0.46 M (784) | 4.6x |
| LiveRangeCalc::extend | 8.0 M (2918) | in finish/LiveIntervals | |
| RAGreedy::run vs select_or_split_impl + enqueue + assign + hoist + weights | ~62 M | 29.1 M | 2.1x |

The pin has no same-block early-out in moveToBlock either (`Entry::get` is a tag check, as ours). Our move_to_block is called 30.7 K times for 8.8 K blocks computed; the pin inlines the hit path into its callers.
