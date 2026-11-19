# m32 file programs: clocks against main and gcc/clang

Measured 2026-11-11 with the tip's vsgcc harness on both sides (`crates/target/llrm-x86-m32/vsgcc`, commit 80712f0135), `-march=i486 -mabi=sysv`, the program's own header flags, -O1/-O2/-O3/-Os. Cells are `clocks (code bytes)`. Rows M32-IO-CLK-1..4 of [above-main-rows](above-main-rows.md) cite this file.

| side | build | llrm-cc / llrm-llc md5 |
|---|---|---|
| tip | release/next a82d9ef8b7 | 84aa5ac21c0e / 0905e7a2b586 |
| main | 65098fd950 release dist; a build from source gives identical objects (div_constants_long, 4 levels) | 51a238d94bf0 / a1d1a9ae2efa |
| gcc, clang | 13.4.0; the pin d1106deb71cc | same objects in both tables |

Inputs: SIGNAL (131,072 B, `tools/dosbatch/signal_gen`), `bench/jpeg_single/photo.jpg`, the first 1,048,576 B of dickens for grep (`ci_bytes`, answer `grep.ci.out`; the full 10 MB would pass the harness's 300M-instruction cap). The file API is one shim for every compiler (`bench_io_shim.h`): the same number of calls in every column of a program (stft 1028, jpeg 221, grep 1025), each priced as its `ret`, the work behind it at 0.

Checked: every column of a program prints its `.out`, writes files of one md5 (stft_fixed SPECTRA 0449f8d6ce17, stft_single 1653af4c0ad9, jpeg_fixed OUT.BMP 969d613dc719, jpeg_single 89e49c23ff98) and those equal a native `gcc -m32` run of the same program. A cell that did not read its input would print -1 early; none does (`failed_opens` 0).

## stft_fixed

| level | main | tip | gcc | clang | tip vs main | tip vs best of gcc/clang |
|---|--:|--:|--:|--:|--:|--:|
| O1 | 97,661,954 (1092) | 93,956,937 (1025) | 96,279,693 (1172) | 94,822,329 (1007) | -3.8% | -0.9% |
| O2 | 97,644,290 (1105) | 93,933,587 (1093) | 90,548,049 (1103) | 94,804,665 (1020) | -3.8% | +3.7% |
| O3 | 97,644,290 (1105) | 93,933,587 (1093) | 90,602,704 (1189) | 94,804,665 (1020) | -3.8% | +3.7% |
| Os | 96,753,648 (1055) | 188,923,280 (928) | 186,499,141 (966) | 94,873,661 (988) | +95.3% | +99.1% |

## stft_single

| level | main | tip | gcc | clang | tip vs main | tip vs best of gcc/clang |
|---|--:|--:|--:|--:|--:|--:|
| O1 | 155,520,040 (963) | 158,226,667 (870) | 157,881,355 (962) | 162,275,327 (884) | +1.7% | +0.2% |
| O2 | 155,502,376 (976) | 150,369,755 (899) | 157,546,520 (948) | 162,257,663 (897) | -3.3% | -4.6% |
| O3 | 155,502,376 (976) | 150,369,755 (899) | 157,459,627 (1005) | 162,257,663 (897) | -3.3% | -4.5% |
| Os | 154,355,208 (940) | 158,420,348 (823) | 163,969,536 (849) | 160,593,855 (866) | +2.6% | -1.4% |

## grep

| level | main | tip | gcc | clang | tip vs main | tip vs best of gcc/clang |
|---|--:|--:|--:|--:|--:|--:|
| O1 | 34,712,679 (609) | 29,426,857 (520) | 23,316,799 (585) | 28,436,914 (527) | -15.2% | +26.2% |
| O2 | 30,537,717 (632) | 29,427,007 (562) | 28,417,132 (562) | 28,437,279 (514) | -3.6% | +3.6% |
| O3 | 29,487,415 (687) | 29,425,028 (571) | 28,417,000 (538) | 28,435,219 (533) | -0.2% | +3.5% |
| Os | 31,584,156 (585) | 33,612,904 (465) | 30,525,175 (429) | 30,469,717 (484) | +6.4% | +10.3% |

## jpeg_fixed

| level | main | tip | gcc | clang | tip vs main | tip vs best of gcc/clang |
|---|--:|--:|--:|--:|--:|--:|
| O1 | 52,754,165 (7684) | 50,099,379 (4563) | 72,318,155 (3579) | 52,048,081 (6731) | -5.0% | -3.7% |
| O2 | 48,014,082 (9919) | 33,486,890 (6979) | 50,005,135 (3702) | 46,782,354 (8866) | -30.3% | -28.4% |
| O3 | 47,646,184 (10867) | 33,062,104 (8014) | 32,712,700 (10799) | 46,546,807 (9354) | -30.6% | +1.1% |
| Os | 51,607,232 (3893) | 84,467,665 (3103) | 100,510,263 (2970) | 51,414,969 (3350) | +63.7% | +64.3% |

## jpeg_single

| level | main | tip | gcc | clang | tip vs main | tip vs best of gcc/clang |
|---|--:|--:|--:|--:|--:|--:|
| O1 | 78,043,151 (7700) | 75,007,468 (4596) | 78,475,190 (3709) | 84,939,972 (6934) | -3.9% | -4.4% |
| O2 | 63,662,556 (9861) | 61,848,317 (6376) | 77,378,401 (3711) | 75,015,649 (9211) | -2.8% | -17.6% |
| O3 | 63,686,889 (11982) | 59,856,974 (9386) | 62,095,072 (12733) | 75,588,905 (11880) | -6.0% | -3.6% |
| Os | 75,731,565 (3925) | 80,281,383 (3160) | 81,718,947 (3112) | 82,918,443 (3525) | +6.0% | -1.8% |

