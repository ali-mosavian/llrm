# cfront --opt against bcc -Ox

Measured 2026-09-15 on pal, qglsurf and ls, from `_TEXT` LEDATA of both
objects (llrm's OMF reader, iced decode). bcc flags: `-3 -f87 -mm -Ox -B`.

| module  | bcc bytes | ours | bcc insns | ours |
|---------|----------:|-----:|----------:|-----:|
| pal     | 228       | 331  | 81        | 113  |
| qglsurf | 226       | 246  | 81        | 76   |
| ls      | 1941      | 2267 | 785       | 754  |

bcc's bytes leave out two helpers we inline: the struct copy in
`pal_install` and `F_FTOL@` in `ls_animate`.

## Better

- `pal_bestfit` loop: 27 instructions per iteration on both sides, 7 memory
  operands against bcc's 17. We hoist the parameters' `movsx` and keep `d` in
  a register; bcc reloads and stores dr, dg, db and d every pass.
- DX:AX joined into EAX as `push dx; push ax; pop eax`, 4 bytes against 9.

## Worse, by cause

1. Loop counters left in memory in `pal_install` and `ls_animate`
   (`mov bx,[bp-4]; add bx,1; mov [bp-4],bx`), while `pal_bestfit`'s is in
   `cx`. Cause not yet found.
2. No pointer induction. bcc steps `si` and tests it at the bottom;
   `ls_animate` recomputes `ls + i*8` three times a pass and reloads `ls`,
   and `pal_install` reloads and splits its far pointer twice a pass.
3. Loops not rotated: two branches taken per pass against one.
4. The raise's widening shapes survive: `movzx cx,byte [bx]; movzx ecx,cx`.
5. Two-address copies: `mov edi,ecx; imul edi,ecx` where `imul ecx,ecx`
   would do.
6. Frame traffic: `pal_current` builds its far pointer in a slot, loads it
   32-bit and splits it (33 bytes against 6); qglsurf stores a dead return
   slot on every exit because `&n` makes its frame non-private.
7. Encoding: `push esi` for `si`, `mov eax,0` for `xor`, `cmp bx,0` for
   `or`, no `leave`, early returns jump to a shared epilogue.

1-3 cost per iteration; 4-7 cost bytes.
