; f32 comparisons for the 6502: int __cmpsf2 __eqsf2 __nesf2 __ltsf2 __lesf2 (1 when unordered) and __gesf2 __gtsf2 (-1 when
; unordered), the compiler-rt contract of lib/fp/f32/cmp.c. llrm-6502 A:X-first layout (as sf32_abi.s): a in A:X and rc2:rc3, b in
; rc4:rc5 and rc6:rc7. Result -1, 0 or 1 in A:X. Scratch rc16..rc19. The method is cmp64.s's.

.setcpu "6502"
.export __cmpsf2, __eqsf2, __nesf2, __ltsf2, __lesf2, __gesf2, __gtsf2

rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
un = $90                ; rc16: the result for an unordered pair
tmp = $91               ; rc17
b2 = $92                ; rc18: b2 and b3, moved from rc6:rc7
b3 = $93                ; rc19

.segment "CODE"

__cmpsf2:
__eqsf2:
__nesf2:
__ltsf2:
__lesf2:
    ldy #1
    bne common
__gesf2:
__gtsf2:
    ldy #$FF
common:
    sty un
    ldy rc6                 ; the A:X-first layout into the cells the body reads: a in rc2..rc5, b0:b1 in rc6:rc7, b2:b3 in b2:b3
    sty b2
    ldy rc7
    sty b3
    ldy rc5
    sty rc7
    ldy rc4
    sty rc6
    ldy rc3
    sty rc5
    ldy rc2
    sty rc4
    sta rc2
    stx rc3
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
    lda b3
    and #$7F
    cmp #$7F
    bne b_ok
    lda b2
    bpl b_ok
    and #$7F
    ora rc7
    ora rc6
    beq b_ok
    jmp unordered
b_ok:
    lda b3
    eor rc5
    bmi signs               ; the signs differ
    lda rc5                 ; the same sign: the magnitudes, top byte first
    cmp b3
    bne decided
    lda rc4
    cmp b2
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
    lda b3
    and #$7F
    ora tmp
    ora b2
    beq equal
    lda rc5
    bmi minus1
    bpl plus1
unordered:
    lda un
    bmi minus1
    bpl plus1
