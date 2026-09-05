# What CSE needs, measured on lngmix

`s = s + v \ 7 + v MOD 7` in a loop over operands that never change. 1,844
against a target of 210, the worst on the board with lngmxx.

x86 yields quotient and remainder from one `idiv`, so this is one divide.
We emit two, and neither leaves the loop.

## The two divides are the same computation, and MIR cannot see it

```
  0x004e  load     v1_4        <- [v]
  0x0052  copy     v2_3        <- 7
  0x0058  convert  v3_3        <- v1_4
  0x005a  div      v1_5, v3_4  <- v3_3, v1_4, v2_3     quotient kept
  ...
  0x0079  div      v1_7, v3_7  <- v3_6, v1_6, v2_4     remainder kept
```

`v1_6` and `v2_4` are not `v1_4` and `v2_3`, so no value numbering can
match them. They are the same numbers, pushed and popped:

```
  0x0061  arg #0          st [sp-0x2]
  0x0063  arg #7          st [sp-0x4]
  0x0065  arg [v_hi]      st [sp-0x6]
  0x0069  arg [v_lo]      st [sp-0x8]
  0x0073  result v1_6:4   ld [sp-0x8]     the pair at sp-8 and sp-6 -- v
  0x0075  result v2_4:4   ld [sp-0x4]     the pair at sp-4 and sp-2 -- 7
```

The slots make the identity exact, so a pass could recover it. But the
round trip is not in the program: it is what `calls.py` leaves behind when
the **machine arm** absorbs `B$DVI4` and `B$RMI4` into `idiv`. It emits the
arguments as pushes and pops them back, because at that layer there are no
values to hand the instruction.

## So the order is phase D first

Absorption at the raise -- the plan's phase D -- emits `DIVIDE` over the
argument *values* and the round trip never exists. Then the two divides are
the same key by construction, cse matches them, and the hoist takes the
whole thing out of the loop, which it currently refuses because
`_invariant_run` will not touch a loop holding a call.

Writing a stack-propagation pass instead would work around a round trip
that phase D deletes. lngmix and lngmxx are 8.8x and 8.9x and both are this
program; fpcse and fpcsex at 3.3x and 3.4x are cse without the round trip
and would come first if a smaller step is wanted.
