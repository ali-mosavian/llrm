# 6502: a constant multiple of a narrow counter steps (LU19, 2026-11-12)

dist-F (#9293 head) to dist-G, same harness, 4-pad mean.

| cell | level | bytes | clocks |
|---|---|---|---|
| frameless | O1 | 2462 -> 2450 | 74306 -> 70869 |
| frameless | O2 | 3819 -> 3807 | 68531 -> 65082 |
| huffman_decode | O1 | 1264 -> 1227 | 112717 -> 109612 |
| huffman_decode | O2 | 1355 -> 1333 | 112018 -> 109222 |
| idioms | O1 | 3140 -> 3115 | 13009591 -> 12997181 |
| idioms | O2 | 12199 -> 12174 | 12749302 -> 12761951 |
| split_loops | O1 | 630 -> 632 | 38235 -> 38338 |

Against release/next before #9293 every cell above is better on both axes
(split_loops O1 632 -> 632 B, 40128 -> 38338 clk; idioms O2 -385 B, -10.7% clk).
Os: no cell changes.
