; i64 __ashrdi3(i64 x, int n).
; 64-bit shift of the 6502, llrm-6502 A:X-first layout: x0:x1 in A:X, x2..x7 in rc2..rc7, n at the caller's stack (rc0:rc1)+0, 0 <= n < 64. See shift64.inc.

.setcpu "6502"
.export __ashrdi3
.include "shift64.inc"

.segment "CODE"

__ashrdi3:
    SHIFT64_ENTER
    lda v+7                 ; fill = $FF for a negative x, else 0
    asl a
    lda #0
    sbc #0
    eor #$FF
    sta fill
    cpx #0
    beq sbits
sbyte:
    ldy #0
:   lda v+1,y
    sta v,y
    iny
    cpy #7
    bne :-
    lda fill
    sta v+7
    dex
    bne sbyte
sbits:
    ldx cnt
    beq sout
sbit:
    lda v+7
    cmp #$80                ; the sign into the carry
    ror v+7
    ror v+6
    ror v+5
    ror v+4
    ror v+3
    ror v+2
    ror v+1
    ror v
    dex
    bne sbit
sout:
    SHIFT64_LEAVE
