# Rows tied with SDCC: the sequence is the ISA minimum

Convention: byte arguments `a l h e d c b`, word arguments `hl de bc`, byte result `a`, word result `de`. B = bytes, T = T-states.
Any instruction is at least 1 B and 4 T; `ret` is 1 B and 10 T. The call and driver cost is the same on both sides.

| row | ours | why nothing is shorter or faster |
|---|---|---|
| zero | `xor a; ret`: 2 B, 14 T | The result must be written to `a` (one instruction, 1 B, 4 T) and the function must return (`ret`). |
| add8 | `add a,l; ret`: 2 B, 14 T | One instruction forms the sum in `a`, the result register. |
| add16 | `add hl,de; ex de,hl; ret`: 3 B, 25 T | Only `add hl,rr` (11 T) adds 16 bits, and it leaves the sum in `hl`; the result register is `de`, so one move (`ex de,hl`, 4 T) is needed. The 8-bit route (`ld/add/ld/ld/adc/ld`) is 6 instructions. |
| max16 | `ld a,e; sub l; ld a,d; sbc a,h; ret nc; ex de,hl; ret`: 7 B, 16 T compare | The compare needs 4 B and 16 T: it must not destroy `hl` or `de`, and the only other 16-bit compare, `or a; sbc hl,de; add hl,de`, is 4 B and 30 T. One conditional return, one move to `de`, one `ret`. The `sbc`-based form (`or a; sbc hl,de; ret c; add hl,de; ex de,hl; ret`) is also 7 B but 49 T against 35 T when `hl` wins and 30 T against 27 T when `de` wins. |
