# Pre-RA MachineCSE: row losses (2026-10-19)

Cause of every row: the allocator spills a shared cheap value where the pin rematerializes it. Base: phi-placement head; 818 C/QB/Nib objects.

## R1-R15: -Os bytes (larger)

1. `bench/grep/grep.c` (-m32 -march=i486): 967 to 979
2. `tests/run/c/zz_f.c` (-m16 -march=i386): 297 to 308
3. `tests/run/c/os_files.c` (-m16 -march=i386): 638 to 646
4. `tests/run/c/float_in_place_update.c` (-m16 -march=i386): 284 to 291
5. `bench/grep/grep.c` (-m16 -march=i386): 945 to 950
6. `tests/run/c/double_to_uint64.c` (-m16 -march=i386): 1094 to 1099
7. `tests/run/c/compare_to_int64.c` (-m16 -march=i386): 405 to 409
8. `tests/run/c/register_upper_half.c` (-m16 -march=i386): 291 to 294
9. `bench/bintree/bintree.c` (-m32 -march=i486): 394 to 396
10. `tests/run/c/byval_struct.c` (-m32 -march=i486): 331 to 333
11. `tests/run/c/os_more.c` (-m16 -march=i386): 280 to 282
12. `tests/run/c/shrink_wrapped_frame.c` (-m16 -march=i386): 367 to 369
13. `bench/huge/huge.c` (-m16 -march=i386): 81403 to 81404
14. `tests/run/c/byval_struct.c` (-m16 -march=i386): 358 to 359
15. `tests/run/c/register_upper_half.c` (-m32 -march=i486): 265 to 266

## R16-: executed instructions (O2/O3, more)

16. `bench/grep/grep.c` (-O2 -m16 -march=i386): 46478 to 47502
17. `bench/jpeg_fixed/jpeg_fixed.c` (-O2 -m16 -march=i386): 4004739 to 4013054
18. `bench/jpeg_single/jpeg_single.c` (-O2 -m16 -march=i386): 4002530 to 4010752
19. `tests/run/c/byval_struct.c` (-O2 -m16 -march=i386): 267 to 268
20. `tests/run/c/double_to_uint64.c` (-O2 -m16 -march=i386): 442 to 444
21. `tests/run/c/idiom_edges.c` (-O2 -m16 -march=i386): 7734 to 7738
22. `tests/run/c/register_upper_half.c` (-O2 -m16 -march=i386): 49 to 50
23. `tests/run/c/zz_f.c` (-O2 -m16 -march=i386): 27 to 28
24. `bench/grep/grep.c` (-O2 -m32 -march=i486): 41480 to 44488
25. `tests/run/c/bench_io.c` (-O2 -m32 -march=i486): 2010 to 2040
26. `tests/run/c/byval_struct.c` (-O2 -m32 -march=i486): 221 to 223
27. `tests/run/c/frame_address_stack_args.c` (-O2 -m32 -march=i486): 69 to 70
28. `tests/run/c/idiom_edges.c` (-O2 -m32 -march=i486): 8052 to 8058
29. `tests/run/c/idioms.c` (-O2 -m32 -march=i486): 12350 to 12372
30. `tests/run/c/register_upper_half.c` (-O2 -m32 -march=i486): 44 to 45
31. `tests/run/c/shrink_wrapped_frame.c` (-O2 -m32 -march=i486): 65 to 66
32. `bench/grep/grep.c` (-O3 -m16 -march=i386): 45339 to 46363
33. `tests/run/c/byval_struct.c` (-O3 -m16 -march=i386): 53 to 54
34. `tests/run/c/double_to_uint64.c` (-O3 -m16 -march=i386): 442 to 444
35. `tests/run/c/register_upper_half.c` (-O3 -m16 -march=i386): 49 to 50
36. `tests/run/c/spill_store_order.c` (-O3 -m16 -march=i386): 145220 to 145221
37. `tests/run/c/zz_f.c` (-O3 -m16 -march=i386): 27 to 28
38. `bench/grep/grep.c` (-O3 -m32 -march=i486): 40340 to 43348
39. `tests/run/c/bench_io.c` (-O3 -m32 -march=i486): 2010 to 2040
40. `tests/run/c/byval_struct.c` (-O3 -m32 -march=i486): 49 to 51
41. `tests/run/c/frame_address_stack_args.c` (-O3 -m32 -march=i486): 69 to 70
42. `tests/run/c/idioms.c` (-O3 -m32 -march=i486): 14751 to 14754
43. `tests/run/c/register_upper_half.c` (-O3 -m32 -march=i486): 44 to 45
44. `tests/run/c/shrink_wrapped_frame.c` (-O3 -m32 -march=i486): 65 to 66
45. `tests/run/c/spill_store_order.c` (-O3 -m32 -march=i486): 52654 to 52655
