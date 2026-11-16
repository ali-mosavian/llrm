; u64 __ashldi3(u64 x, int n).
; 64-bit shift of the 6502, llrm-6502 A:X-first layout: x0:x1 in A:X, x2..x7 in rc2..rc7, n at the caller's stack (rc0:rc1)+0, 0 <= n < 64. See shift64.inc.

.setcpu "6502"
.export __ashldi3, __ashldi3_a
.include "shift64.inc"

.segment "CODE"

__ashldi3:
    SHIFT64_LOAD
__ashldi3_a:
    SHIFT64_COUNT
    beq lbits
lbyte:
    ldy #7
:   lda v-1,y
    sta v,y
    dey
    bne :-
    sty v
    dex
    bne lbyte
lbits:
    ldx cnt
    beq lout
lbit:
    asl v
    rol v+1
    rol v+2
    rol v+3
    rol v+4
    rol v+5
    rol v+6
    rol v+7
    dex
    bne lbit
lout:
    SHIFT64_LEAVE
