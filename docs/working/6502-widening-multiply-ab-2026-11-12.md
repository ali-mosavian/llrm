# 6502: a product of two words is one 16 x 16 -> 32 multiply (-O2/-O3, 2026-11-12)

dist-G (#9299 head) to dist-J, same harness, 4-pad mean. -O1 and -Os: no cell changes.

| cell | bytes | clocks |
|---|---|---|
| idioms | 12174 -> 12266 | 12761951 -> 9707437 |
| int64_division | 4044 -> 4106 | 994153 -> 921178 |
| loop_versioning | 943 -> 955 | 793872 -> 633224 |
| overflow_builtins | 8043 -> 7769 | 425914 -> 354399 |
| float_in_memory_loop | 363 -> 342 | 664 -> 664 |
| fcmp_all | 37022 -> 36618 | 1060691 -> 967806 |
| int64_divmod_edges | 9531 -> 9365 | 18108103 -> 16076929 |
| mul_words_to_long | 2144 -> 2107 | 10187 -> 10187 |

The three byte rises are rate-better at -O2 (clock + 3 B): +276 against -3.05M, +186 against -73K,
+36 against -160K.
