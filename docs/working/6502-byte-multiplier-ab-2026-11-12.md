# 6502: a product by a byte stops at its last set bit (-O2, 2026-11-12)

dist-J (#9302 head) to dist-L, same harness, 4-pad mean. -O1 and -Os: no cell changes.

| cell | bytes | clocks |
|---|---|---|
| idioms | 12266 -> 12208 | 9707437 -> 8764472 |
| loop_versioning | 955 -> 897 | 633224 -> 576650 |
| fcmp_all | 36618 -> 36597 | 967806 -> 897910 |
