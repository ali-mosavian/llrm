# commute-load clock rows (#3874)

Stage diff (`ISEL_DUMP=1`, -O1 -m32 -march=i486, base 845e8eb919 vs 7a2a49f5c1): the first stage that differs is TwoAddress, in both programs. Every `imul` of a loaded value swaps its tied operand (TA-8):

    -  v57 := mov v56 ; v57 := imul v57, v26      (v56 loaded: tied)
    +  v57 := mov v26 ; v57 := imul v57, v56      (v26 tied)

- **quicksort** (+4.6..5.5%): one object difference, the checksum loop `imul ebx, edx` becomes `imul edx, ebx`. The rows' clock model prices IMUL by its multiplier operand (tools/bench/flatcount.py:231); the multiplier is now the loaded value (large) instead of `(i & 15) + 1` (small). Cost read from the row delta: 213722 - 203055 = 10667 clocks over 1024 iterations, about 10 per iteration.
- **bintree** (+1..2%): the same swap, then the allocator spills one more value (one function's frame goes 32 -> 36 bytes); a downstream pass, not TwoAddress.

Not compared with the pin's own choice for these `imul`s; TA-8's exit (loads folded into their reader before the pass) removes the case.
