# 6502: a literal stored while a is live is stored where a is dead (2026-11-12)

dist-J (#9302 head) to dist-K, same harness, 4-pad mean. Bytes only fall.

| cell | level | bytes | clocks |
|---|---|---|---|
| word_compares | O1 | 485 -> 481 | 95014 -> 92296 |
| word_compares | O2 | 485 -> 481 | 95014 -> 92296 |
| word_compares | Os | 491 -> 487 | 104409 -> 101666 |
| copy_runs | O1 | 2259 -> 2255 | 71012 -> 70468 |
| copy_runs | O2 | 2374 -> 2370 | 70691 -> 70184 |
| copy_runs | Os | 1446 -> 1444 | 410512 -> 410176 |
| idioms | O2 | 12266 -> 11800 | 9707437 -> 9738872 |
| idioms | Os | 3083 -> 3081 | 30578637 -> 30599799 |
| sat_builtins | Os | 2035 -> 2027 | 33936 -> 33878 |
| overflow_builtins | Os | 8086 -> 8066 | 723023 -> 725042 |
| fcmp_all | O2 | 36618 -> 36604 | 967806 -> 967900 |

The clock rises are layout: executed instructions fall (idioms Os 6,984,059 -> 6,964,859,
overflow_builtins Os 163,963 -> 163,921), three of four pad layouts are faster, one crosses pages.
idioms O2 is rate-better (-466 B for +31K clk).
