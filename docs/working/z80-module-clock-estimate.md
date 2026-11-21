# Z80 module price: static clock estimate runs fast on smaller modules

Owner: z80. Opened 2026-11-21 (oversight ruling: a cell under both SDCC bars is a note, not a worse-cell row).

`compile::module_priced` ranks module variants bytes first, then clocks (2026-11-14 ruling). Its clock estimate is static (no trip counts), and it ranks the smaller module "no slower" where ucsim runs it 2-5% slower. Measured, all under their SDCC bars (board21):

| cell | bytes | ucsim T | bar |
|---|---|---|---|
| int64_divmod_edges O2/O3 | 3957 -> 3297 | 50.55M -> 51.75M (+2.4%) | 4061 B / 219M T |
| pattern_fill O2/O3 | 422 -> 388 | 289,821 -> 303,547 (+4.7%) | 440 B / 402k T |
| mul_words_to_long O2 | 139 -> 137 | 142,354 -> 145,094 (+1.9%) | 197 B / 156k T |
| memmove_overlap O3 | 487 -> 387 | 30,292 -> 31,789 (+4.9%) | 568 B / 193,531 T (board21 O2) |

Rule: if a later cell crosses a bar because of this estimate, that cell's branch is the finished-module clock fix (price the module with executed-clock weights, not the static count).
