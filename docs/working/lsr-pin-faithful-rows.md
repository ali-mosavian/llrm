# Pin-faithful rows of loop-reduce at m16 -Os

LSR output equal to the pin's `opt -passes=loop-reduce` on the same pre-LSR IR (22-canonfreeze), measured 2026-10-26 at f0fa38dbb; the bytes are the m16 backend's.

| row | program | bytes | cause |
|---|---|--:|---|
| TL-1 | tile.c, tile.nib, tile.bas | +10, +7, +10 | both LSRs make `phi i6` for `(y + 61) & 63`; at m16 its `zext` costs `mov bl, cl; and bl, 63; movzx bx, bl; shl bx, 7` where the old rewrite was one `and si, 8064`. Routed to w10 (isel). |
| TL-2 | crc.c | +9 | identical phis; the inner body is branchy (`neg; jne`) where main was and/neg/and. Routed to w10 (ifcvt). |
