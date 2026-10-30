# Rows of the load forwarding in combine (#5008)

m16 -Os growers of `visit_load`, each a size the pin's pipeline shares or a constant it now folds.

| row | program | bytes | cause |
|---|---|--:|---|
| CL-1 | qb/hugelp.bas | +13 | the 2-trip loop is fully unrolled into four `mov word ptr es:[disp], imm` (7 B each); the pin's full unroll does the same |
| CL-2 | demo-qbdemo | +1 | the counted loop starts at -65: `mov ax, cx; add ax, 256` replace `mov al, cl`, and `mov word ptr X%, 256` is an immediate store |
| CL-3 | demo-deedlines | +3 | `mov I%, cx; dec cx; mov TSC$D34+192, cx` become two immediate stores (`mov I%, 827`; `mov TSC$D34+192, 826`) |

-Oz growers (hugerg +42, pow_stack_depth +9, stride +7) are loops now counted whose LSR form costs more at m16: the LSR register-scarcity row, not these.
