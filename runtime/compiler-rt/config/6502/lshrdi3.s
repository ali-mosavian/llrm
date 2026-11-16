; u64 __lshrdi3(u64 x, int n).
; 64-bit shift of the 6502, llrm-6502 A:X-first layout: x0:x1 in A:X, x2..x7 in rc2..rc7, n at the caller's stack (rc0:rc1)+0, 0 <= n < 64. See shift64.inc.

.setcpu "6502"
.export __lshrdi3
.include "shift64.inc"

.segment "CODE"

__lshrdi3:
    SHIFT64_ENTER
    beq rbits
rbyte:
    ldy #0
:   lda v+1,y
    sta v,y
    iny
    cpy #7
    bne :-
    lda #0
    sta v+7
    dex
    bne rbyte
rbits:
    ldx cnt
    beq rout
rbit:
    lsr v+7
    ror v+6
    ror v+5
    ror v+4
    ror v+3
    ror v+2
    ror v+1
    ror v
    dex
    bne rbit
rout:
    SHIFT64_LEAVE
