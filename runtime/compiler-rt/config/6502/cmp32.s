; f32 comparisons for the 6502: int __cmpsf2 __eqsf2 __nesf2 __ltsf2 __lesf2 (1 when unordered) and __gesf2 __gtsf2 (-1 when
; unordered), the compiler-rt contract of lib/fp/f32/cmp.c. llrm-6502 rc2-first layout (as sf32_abi.s): a in rc2..rc5, b0:b1 in
; rc6:rc7, b2:b3 at the caller's stack (rc0:rc1)+0/+1. Result -1, 0 or 1 in A:X. Scratch rc16, rc17. The method is cmp64.s's.

.setcpu "6502"
.export __cmpsf2, __eqsf2, __nesf2, __ltsf2, __lesf2, __gesf2, __gtsf2

rc0 = $80
rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
un = $90                ; rc16: the result for an unordered pair
tmp = $91               ; rc17

.segment "CODE"

__cmpsf2:
__eqsf2:
__nesf2:
__ltsf2:
__lesf2:
    lda #1
    bne common
__gesf2:
__gtsf2:
    lda #$FF
common:
    sta un
    lda rc5                 ; a NaN: exponent all ones and a mantissa
    and #$7F
    cmp #$7F
    bne a_ok
    lda rc4
    bpl a_ok
    and #$7F
    ora rc3
    ora rc2
    beq a_ok
    jmp unordered
a_ok:
    ldy #1                  ; b3
    lda (rc0),y
    and #$7F
    cmp #$7F
    bne b_ok
    dey
    lda (rc0),y             ; b2
    bpl b_ok
    and #$7F
    ora rc7
    ora rc6
    beq b_ok
    jmp unordered
b_ok:
    ldy #1
    lda (rc0),y
    eor rc5
    bmi signs               ; the signs differ
    lda rc5                 ; the same sign: the magnitudes, top byte first
    cmp (rc0),y
    bne decided
    lda rc4
    dey
    cmp (rc0),y
    bne decided
    lda rc3
    cmp rc7
    bne decided
    lda rc2
    cmp rc6
    bne decided
equal:
    lda #0
    tax
    rts
decided:                    ; C: the magnitude of a is the greater
    bcs greater
    lda rc5                 ; smaller magnitude: less if positive
    bmi plus1
minus1:
    lda #$FF
    tax
    rts
greater:
    lda rc5
    bmi minus1
plus1:
    lda #1
    ldx #0
    rts
signs:                      ; two zeros are equal whatever their signs; otherwise the negative one is less
    lda rc5
    and #$7F
    ora rc4
    ora rc3
    ora rc2
    ora rc6
    ora rc7
    sta tmp
    ldy #1
    lda (rc0),y
    and #$7F
    ora tmp
    dey
    ora (rc0),y
    beq equal
    lda rc5
    bmi minus1
    bpl plus1
unordered:
    lda un
    bmi minus1
    bpl plus1
